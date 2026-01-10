use crate::cli::Cli;
use crate::daemon_client;
use crate::desktop::scan_and_parse_desktop_files;
use crate::frequency::FrequencyStore;
use crate::ipc::{Request, Response};
use crate::launch::{Terminal, exec_to_argv, pick_terminal};

use super::common::{timing, trace};

pub fn launch(
    cli: &Cli,
    scan_roots: &[std::path::PathBuf],
    desktop_id: &str,
    action: Option<&str>,
) -> i32 {
    let start = std::time::Instant::now();
    let roots: Vec<String> = scan_roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    if !cli.no_daemon
        && let Some(resp) = daemon_client::try_request(&Request::Launch {
            roots,
            desktop_id: desktop_id.to_string(),
            action: action.map(|s| s.to_string()),
            respect_try_exec: cli.respect_try_exec,
        })
    {
        match resp {
            Response::Ok => {
                trace(cli, "mode=daemon (launch)");
                timing("daemon", start);
                return 0;
            }
            Response::Error { message } => {
                eprintln!("desktop-indexer: daemon error: {message} (fallback local)");
            }
            _ => {}
        }
    }

    trace(cli, "mode=local (launch)");
    timing("local", start);

    // Local fallback
    use std::process::Command;
    let id = desktop_id.trim_end_matches(".desktop");

    let mut freqs = FrequencyStore::load();

    let result = scan_and_parse_desktop_files(scan_roots, None, cli.respect_try_exec);
    let entry = result.entries.iter().find(|e| e.out.id == id);
    let Some(entry) = entry else {
        eprintln!("Unknown desktop-id: {id}");
        return 1;
    };

    let mut selected_exec: Option<&str> = entry.out.exec.as_deref();
    if let Some(action_id) = action {
        let Some(act) = entry.out.actions.iter().find(|a| a.id == action_id) else {
            eprintln!("Unknown action '{action_id}' for id={id}");
            if !entry.out.actions.is_empty() {
                eprintln!("Available actions:");
                for a in &entry.out.actions {
                    eprintln!("  {}", a.id);
                }
            }
            return 1;
        };
        selected_exec = act.exec.as_deref();
    }

    if action.is_none() {
        let gtk_status = Command::new("gtk-launch").arg(id).status();
        match gtk_status {
            Ok(s) if s.success() => {
                freqs.increment(id);
                freqs.flush();
                return 0;
            }
            Ok(_) | Err(_) => {}
        }
    }

    if entry.out.terminal {
        let Some(exec_line) = selected_exec else {
            eprintln!("Terminal app but no Exec= for id={id}");
            return 1;
        };

        let argv = exec_to_argv(exec_line);
        if argv.is_empty() {
            eprintln!("Exec parsed empty for id={id} (Exec={exec_line})");
            return 1;
        }

        let term = pick_terminal();
        match term {
            Some(Terminal::Foot) => {
                let mut cmd = Command::new("foot");
                cmd.arg("-e").arg(&argv[0]).args(&argv[1..]);
                let _ = cmd
                    .spawn()
                    .map_err(|e| eprintln!("Failed to spawn foot: {e}"));
                freqs.increment(id);
                freqs.flush();
                return 0;
            }
            Some(Terminal::Kitty) => {
                let mut cmd = Command::new("kitty");
                cmd.arg(&argv[0]).args(&argv[1..]);
                let _ = cmd
                    .spawn()
                    .map_err(|e| eprintln!("Failed to spawn kitty: {e}"));
                freqs.increment(id);
                freqs.flush();
                return 0;
            }
            Some(Terminal::Alacritty) => {
                let mut cmd = Command::new("alacritty");
                cmd.arg("-e").arg(&argv[0]).args(&argv[1..]);
                let _ = cmd
                    .spawn()
                    .map_err(|e| eprintln!("Failed to spawn alacritty: {e}"));
                freqs.increment(id);
                freqs.flush();
                return 0;
            }
            Some(Terminal::WezTerm) => {
                let mut cmd = Command::new("wezterm");
                cmd.args(["start", "--"]).arg(&argv[0]).args(&argv[1..]);
                let _ = cmd
                    .spawn()
                    .map_err(|e| eprintln!("Failed to spawn wezterm: {e}"));
                freqs.increment(id);
                freqs.flush();
                return 0;
            }
            None => {
                eprintln!("gtk-launch failed and no known terminal found for Terminal=true app.");
                eprintln!("Install one of: foot, kitty, alacritty, wezterm");
                return 1;
            }
        }
    }

    let Some(exec_line) = selected_exec else {
        eprintln!("Launch failed and no Exec= for id={id}");
        return 1;
    };

    let argv = exec_to_argv(exec_line);
    if argv.is_empty() {
        eprintln!("Exec parsed empty for id={id} (Exec={exec_line})");
        return 1;
    }

    let mut cmd = Command::new(&argv[0]);
    if argv.len() > 1 {
        cmd.args(&argv[1..]);
    }

    let _ = cmd
        .spawn()
        .map_err(|e| eprintln!("Exec launch failed for id={id}: {e}"));

    freqs.increment(id);
    freqs.flush();

    0
}
