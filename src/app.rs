use crate::cli::{Cli, Cmd, DaemonCmd};
use crate::commands;

pub fn run(cli: Cli) -> i32 {
    // Resolve scan roots from XDG + -p paths
    let scan_roots = crate::xdg::build_scan_roots(&cli.paths);

    match &cli.cmd {
        Cmd::Daemon { cmd } => match cmd {
            DaemonCmd::Start => commands::daemon::start_daemon(&cli, &scan_roots),
            DaemonCmd::Stop => commands::daemon::stop_daemon(&cli),
            DaemonCmd::Restart => commands::daemon::restart_daemon(&cli, &scan_roots),
            DaemonCmd::Status { json } => commands::status::status(&cli, *json),
        },
        Cmd::StartDaemon => commands::daemon::start_daemon(&cli, &scan_roots),
        Cmd::StopDaemon => commands::daemon::stop_daemon(&cli),
        Cmd::RunDaemon => commands::daemon::run_daemon(),
        Cmd::Status { json } => commands::status::status(&cli, *json),
        Cmd::Scan { limit, parse, json } => {
            commands::scan::scan(&scan_roots, *limit, *parse, *json, cli.respect_try_exec)
        }
        Cmd::Search {
            query,
            limit,
            empty_mode,
            json,
        } => commands::search::search(&cli, &scan_roots, query, *limit, *empty_mode, *json),
        Cmd::List { json } => commands::list::list(&cli, &scan_roots, *json),
        Cmd::Parse { path, json } => commands::parse::parse(&scan_roots, path, *json),
        Cmd::Launch { desktop_id, action } => {
            commands::launch::launch(&cli, &scan_roots, desktop_id, action.as_deref())
        }
    }
}
