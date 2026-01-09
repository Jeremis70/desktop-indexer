use crate::cli::Cli;
use crate::ipc::{Request, Response};
use crate::{daemon, daemon_client};

use super::common::trace;

pub fn start_daemon(cli: &Cli, scan_roots: &[std::path::PathBuf]) -> i32 {
    match daemon::start_daemon() {
        Ok(daemon::StartResult::Started) => {
            warmup_daemon(cli, scan_roots);
            println!("daemon started successfully");
            0
        }
        Ok(daemon::StartResult::AlreadyRunning) => {
            warmup_daemon(cli, scan_roots);
            println!("daemon already started");
            0
        }
        Err(e) => {
            eprintln!("daemon start error: {e}");
            1
        }
    }
}

fn warmup_daemon(cli: &Cli, scan_roots: &[std::path::PathBuf]) {
    if cli.no_daemon {
        return;
    }

    let roots: Vec<String> = scan_roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let resp = daemon_client::try_request(&Request::Warmup { roots });
    if matches!(resp, Some(Response::Ok)) {
        trace(cli, "daemon warmup ok");
    } else {
        trace(cli, "daemon warmup failed");
    }
}

pub fn stop_daemon(cli: &Cli) -> i32 {
    if cli.no_daemon {
        eprintln!("desktop-indexer: --no-daemon set; not stopping daemon");
        return 0;
    }

    match daemon_client::try_request(&Request::Shutdown) {
        Some(Response::Ok) => {
            println!("daemon stopped");
            0
        }
        Some(Response::Error { message }) => {
            if message.contains("unknown variant `shutdown`") {
                eprintln!(
                    "desktop-indexer: daemon is running but too old (no shutdown support). Restart it manually, then try again."
                );
            }
            eprintln!("desktop-indexer: daemon error: {message}");
            1
        }
        _ => {
            println!("daemon not running");
            0
        }
    }
}

pub fn run_daemon() -> i32 {
    if let Err(e) = daemon::run_daemon_foreground() {
        eprintln!("desktop-indexer: daemon failed: {e}");
        return 1;
    }
    0
}
