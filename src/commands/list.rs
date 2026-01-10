use crate::cli::Cli;
use crate::daemon_client;
use crate::desktop::scan_and_parse_desktop_files;
use crate::ipc::{Request, Response};
use crate::models::DesktopEntryOut;
use crate::output::print_json;

use super::common::{timing, trace};

pub fn list(cli: &Cli, scan_roots: &[std::path::PathBuf], json: bool) -> i32 {
    let start = std::time::Instant::now();
    let roots: Vec<String> = scan_roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let daemon_resp = if cli.no_daemon {
        None
    } else {
        daemon_client::try_request(&Request::List {
            roots,
            respect_try_exec: cli.respect_try_exec,
        })
    };

    let (mode, mut entries): (&str, Vec<DesktopEntryOut>) = if let Some(resp) = daemon_resp {
        match resp {
            Response::Entries { entries } => ("daemon", entries),
            Response::Error { message } => {
                eprintln!("desktop-indexer: daemon error: {message} (fallback local)");
                let result = scan_and_parse_desktop_files(scan_roots, None, cli.respect_try_exec);
                ("local", result.entries.into_iter().map(|e| e.out).collect())
            }
            _ => {
                let result = scan_and_parse_desktop_files(scan_roots, None, cli.respect_try_exec);
                ("local", result.entries.into_iter().map(|e| e.out).collect())
            }
        }
    } else {
        let result = scan_and_parse_desktop_files(scan_roots, None, cli.respect_try_exec);
        ("local", result.entries.into_iter().map(|e| e.out).collect())
    };

    entries.sort_by(|a, b| {
        a.name
            .as_deref()
            .unwrap_or("")
            .cmp(b.name.as_deref().unwrap_or(""))
    });

    trace(cli, &format!("mode={mode} (list)"));
    timing(mode, start);

    if json {
        print_json(&entries);
    } else {
        for e in &entries {
            println!("{}\t{}", e.id, e.name.as_deref().unwrap_or(""));
        }
    }

    0
}
