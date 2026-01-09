use crate::cli::Cli;
use crate::daemon_client;
use crate::ipc::{Request, Response};
use crate::output::print_json;
use crate::xdg;

use super::common::{timing, trace};

pub fn status(cli: &Cli, json: bool) -> i32 {
    let start = std::time::Instant::now();
    let socket = xdg::socket_path().to_string_lossy().to_string();

    let resp = if cli.no_daemon {
        None
    } else {
        daemon_client::try_request(&Request::Status)
    };

    #[derive(serde::Serialize)]
    struct StatusOut {
        daemon: bool,
        has_index_count: Option<usize>,
        socket: String,
    }

    let (mode, out) = match resp {
        Some(Response::Status { has_index_count }) => (
            "daemon",
            StatusOut {
                daemon: true,
                has_index_count: Some(has_index_count),
                socket,
            },
        ),
        _ => (
            "local",
            StatusOut {
                daemon: false,
                has_index_count: None,
                socket,
            },
        ),
    };

    trace(cli, &format!("mode={mode} (status)"));
    timing(mode, start);

    if json {
        print_json(&out);
    } else if out.daemon {
        println!(
            "daemon running (indexes={})",
            out.has_index_count.unwrap_or(0)
        );
        println!("socket={}", out.socket);
    } else {
        println!("daemon not running");
        println!("socket={}", out.socket);
    }

    0
}
