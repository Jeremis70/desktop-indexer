use crate::desktop::scan_and_parse_desktop_files;
use crate::frequency::FrequencyStore;
use crate::ipc::{Request, Response};
use crate::launch::{Terminal, exec_to_argv, pick_terminal};
use crate::xdg::socket_path;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

struct IndexState {
    entries: Vec<crate::models::DesktopEntryIndexed>,
    last_tokens: Vec<String>,
    last_candidates: Vec<usize>,
    last_query_key: String,
}

fn query_key(query: &str) -> String {
    // A simple normalization for typeahead refinement checks.
    // Lowercase + trim + collapse whitespace.
    let mut out = String::new();
    let mut prev_ws = false;
    for ch in query.trim().chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
            continue;
        }
        prev_ws = false;
        for lc in ch.to_lowercase() {
            out.push(lc);
        }
    }
    out
}

fn tokens_contain_all(tokens: &[String], prev: &[String]) -> bool {
    if prev.is_empty() {
        return false;
    }
    prev.iter().all(|t| tokens.iter().any(|x| x == t))
}

pub fn start_daemon() -> std::io::Result<StartResult> {
    let path = socket_path();

    // Already running?
    if UnixStream::connect(&path).is_ok() {
        return Ok(StartResult::AlreadyRunning);
    }

    // Clean stale socket.
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }

    // Spawn detached child: same binary, internal subcommand.
    let exe = std::env::current_exe()?;
    let mut child = std::process::Command::new(exe);
    child
        .arg("run-daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    let _ = child.spawn()?;

    // Wait briefly for socket to become available.
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(800) {
        if UnixStream::connect(&path).is_ok() {
            return Ok(StartResult::Started);
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    Ok(StartResult::Started)
}

pub enum StartResult {
    Started,
    AlreadyRunning,
}

pub fn run_daemon_foreground() -> std::io::Result<()> {
    let path = socket_path();

    // If socket exists, check if daemon is alive.
    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            eprintln!(
                "desktop-indexer: daemon already running at {}",
                path.display()
            );
            return Ok(());
        }
        let _ = std::fs::remove_file(&path);
    }

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = UnixListener::bind(&path)?;
    eprintln!("desktop-indexer: daemon listening on {}", path.display());

    let mut indexes: HashMap<Vec<String>, IndexState> = HashMap::new();
    let mut freqs = FrequencyStore::load();

    let mut shutdown = false;

    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                shutdown = handle_connection(stream, &mut indexes, &mut freqs);
                if shutdown {
                    break;
                }
            }
            Err(e) => {
                eprintln!("desktop-indexer: accept error: {e}");
            }
        }
    }

    drop(listener);
    if shutdown {
        freqs.flush();
        let _ = std::fs::remove_file(&path);
        eprintln!("desktop-indexer: daemon stopped");
    }

    Ok(())
}

fn handle_connection(
    stream: UnixStream,
    indexes: &mut HashMap<Vec<String>, IndexState>,
    freqs: &mut FrequencyStore,
) -> bool {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return false;
    }

    let req = match serde_json::from_str::<Request>(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            let _ = write_response(
                reader.into_inner(),
                Response::Error {
                    message: format!("invalid request: {e}"),
                },
            );
            return false;
        }
    };

    let (resp, shutdown) = handle_request(indexes, freqs, req);
    let _ = write_response(reader.into_inner(), resp);
    shutdown
}

fn write_response(mut stream: UnixStream, resp: Response) -> std::io::Result<()> {
    let line = serde_json::to_string(&resp).unwrap_or_else(|_| {
        serde_json::to_string(&Response::Error {
            message: "failed to serialize response".to_string(),
        })
        .unwrap()
    });
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

fn handle_request(
    indexes: &mut HashMap<Vec<String>, IndexState>,
    freqs: &mut FrequencyStore,
    req: Request,
) -> (Response, bool) {
    match req {
        Request::Shutdown => {
            freqs.flush();
            (Response::Ok, true)
        }

        Request::Warmup { roots } => {
            if ensure_index(indexes, &roots).is_some() {
                (Response::Ok, false)
            } else {
                (
                    Response::Error {
                        message: "failed to build index".to_string(),
                    },
                    false,
                )
            }
        }

        Request::Status => (
            Response::Status {
                has_index_count: indexes.len(),
            },
            false,
        ),

        Request::Search {
            roots,
            query,
            limit,
            empty_mode,
        } => {
            let Some(state) = ensure_index(indexes, &roots) else {
                return (
                    Response::Error {
                        message: "failed to build index".to_string(),
                    },
                    false,
                );
            };

            let lim = limit.unwrap_or(20);
            let qkey = query_key(&query);
            let tokens = crate::search::normalize_query(&query);
            if tokens.is_empty() {
                let mode = empty_mode.unwrap_or(crate::empty_query::EmptyQueryMode::Recency);
                let entries = crate::search::search_entries_with_usage_map_and_empty_mode(
                    &state.entries,
                    "",
                    lim,
                    freqs.map(),
                    mode,
                );

                state.last_tokens.clear();
                state.last_candidates.clear();
                state.last_query_key.clear();

                return (Response::Entries { entries }, false);
            }

            // Incremental optimization: if the new query is a refinement of the previous
            // one, we can filter the previous candidate set instead of re-scanning the whole index.
            // We treat these as refinements:
            // - token superset ("text" -> "text editor")
            // - typeahead prefix ("v" -> "vs" -> "vsc")
            let is_typeahead_prefix = state.last_tokens.len() == 1
                && tokens.len() == 1
                && !state.last_tokens[0].is_empty()
                && tokens[0].starts_with(&state.last_tokens[0]);

            let is_query_prefix = !state.last_query_key.is_empty()
                && qkey.len() > state.last_query_key.len()
                && qkey.starts_with(&state.last_query_key);

            let can_reuse = tokens_contain_all(&tokens, &state.last_tokens)
                || is_typeahead_prefix
                || is_query_prefix;

            let mut candidates: Vec<usize> = if can_reuse {
                state.last_candidates.clone()
            } else {
                (0..state.entries.len()).collect()
            };

            candidates.retain(|&idx| {
                let e = &state.entries[idx];
                tokens.iter().all(|t| e.norm.contains(t))
            });

            // Score only within candidates (same scoring as search::search_entries).
            use std::{cmp::Reverse, collections::BinaryHeap};
            let mut heap: BinaryHeap<Reverse<(i32, usize)>> = BinaryHeap::new();

            let now_sec = crate::frequency::unix_seconds_now();

            for &idx in &candidates {
                let e = &state.entries[idx];
                let usage = freqs.get(&e.out.id);
                let score = crate::search::score_entry(e, &tokens, usage, now_sec);

                heap.push(Reverse((score, idx)));
                if heap.len() > lim {
                    heap.pop();
                }
            }

            let mut picked: Vec<(i32, usize)> = heap.into_iter().map(|Reverse(x)| x).collect();
            picked.sort_by(|a, b| b.0.cmp(&a.0));

            let entries = picked
                .into_iter()
                .map(|(_, idx)| state.entries[idx].out.clone())
                .collect();

            // Update incremental cache for next query.
            state.last_tokens = tokens;
            state.last_candidates = candidates;
            state.last_query_key = qkey;

            (Response::Entries { entries }, false)
        }

        Request::List { roots } => {
            let Some(state) = ensure_index(indexes, &roots) else {
                return (
                    Response::Error {
                        message: "failed to build index".to_string(),
                    },
                    false,
                );
            };

            let mut entries: Vec<crate::models::DesktopEntryOut> =
                state.entries.iter().map(|e| e.out.clone()).collect();
            entries.sort_by(|a, b| {
                a.name
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.name.as_deref().unwrap_or(""))
            });
            (Response::Entries { entries }, false)
        }

        Request::Launch {
            roots,
            desktop_id,
            action,
        } => {
            let Some(state) = ensure_index(indexes, &roots) else {
                return (
                    Response::Error {
                        message: "failed to build index".to_string(),
                    },
                    false,
                );
            };

            match do_launch(&state.entries, &desktop_id, action.as_deref()) {
                Ok(()) => {
                    let id = desktop_id.trim_end_matches(".desktop");
                    freqs.increment(id);
                    freqs.flush();
                    (Response::Ok, false)
                }
                Err(e) => (Response::Error { message: e }, false),
            }
        }
    }
}

fn ensure_index<'a>(
    indexes: &'a mut HashMap<Vec<String>, IndexState>,
    roots: &[String],
) -> Option<&'a mut IndexState> {
    if !indexes.contains_key(roots) {
        let roots_pb: Vec<PathBuf> = roots.iter().map(PathBuf::from).collect();
        let parsed = scan_and_parse_desktop_files(&roots_pb, None);
        indexes.insert(
            roots.to_vec(),
            IndexState {
                entries: parsed.entries,
                last_tokens: Vec::new(),
                last_candidates: Vec::new(),
                last_query_key: String::new(),
            },
        );
    }
    indexes.get_mut(roots)
}

fn do_launch(
    entries: &[crate::models::DesktopEntryIndexed],
    desktop_id: &str,
    action: Option<&str>,
) -> Result<(), String> {
    let id = desktop_id.trim_end_matches(".desktop");

    let entry = entries
        .iter()
        .find(|e| e.out.id == id)
        .ok_or_else(|| format!("Unknown desktop-id: {id}"))?;

    let mut selected_exec = entry.out.exec.as_deref();
    if let Some(action_id) = action {
        let act = entry
            .out
            .actions
            .iter()
            .find(|a| a.id == action_id)
            .ok_or_else(|| format!("Unknown action '{action_id}' for id={id}"))?;
        selected_exec = act.exec.as_deref();
    }

    // gtk-launch only supports default action
    if action.is_none()
        && let Ok(s) = Command::new("gtk-launch").arg(id).status()
        && s.success()
    {
        return Ok(());
    }

    if entry.out.terminal {
        let exec_line =
            selected_exec.ok_or_else(|| format!("Terminal app but no Exec= for id={id}"))?;
        let argv = exec_to_argv(exec_line);
        if argv.is_empty() {
            return Err(format!("Exec parsed empty for id={id} (Exec={exec_line})"));
        }

        let term = pick_terminal().ok_or_else(|| {
            "gtk-launch failed and no known terminal found for Terminal=true app. Install one of: foot, kitty, alacritty, wezterm".to_string()
        })?;

        match term {
            Terminal::Foot => {
                let mut cmd = Command::new("foot");
                cmd.arg("-e").arg(&argv[0]).args(&argv[1..]);
                cmd.spawn()
                    .map_err(|e| format!("Failed to spawn foot: {e}"))?;
                return Ok(());
            }
            Terminal::Kitty => {
                let mut cmd = Command::new("kitty");
                cmd.arg(&argv[0]).args(&argv[1..]);
                cmd.spawn()
                    .map_err(|e| format!("Failed to spawn kitty: {e}"))?;
                return Ok(());
            }
            Terminal::Alacritty => {
                let mut cmd = Command::new("alacritty");
                cmd.arg("-e").arg(&argv[0]).args(&argv[1..]);
                cmd.spawn()
                    .map_err(|e| format!("Failed to spawn alacritty: {e}"))?;
                return Ok(());
            }
            Terminal::WezTerm => {
                let mut cmd = Command::new("wezterm");
                cmd.args(["start", "--"]).arg(&argv[0]).args(&argv[1..]);
                cmd.spawn()
                    .map_err(|e| format!("Failed to spawn wezterm: {e}"))?;
                return Ok(());
            }
        }
    }

    let exec_line =
        selected_exec.ok_or_else(|| format!("Launch failed and no Exec= for id={id}"))?;
    let argv = exec_to_argv(exec_line);
    if argv.is_empty() {
        return Err(format!("Exec parsed empty for id={id} (Exec={exec_line})"));
    }

    let mut cmd = Command::new(&argv[0]);
    if argv.len() > 1 {
        cmd.args(&argv[1..]);
    }
    cmd.spawn()
        .map_err(|e| format!("Exec launch failed for id={id}: {e}"))?;

    Ok(())
}
