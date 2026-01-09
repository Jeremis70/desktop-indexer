use crate::cli::Cli;
use crate::daemon_client;
use crate::desktop::scan_and_parse_desktop_files;
use crate::empty_query::EmptyQueryMode;
use crate::frequency::FrequencyStore;
use crate::ipc::{Request, Response};
use crate::models::DesktopEntryOut;
use crate::output::print_json;
use crate::search::search_entries_with_usage_map_and_empty_mode;

use super::common::{timing, trace};

pub fn search(
    cli: &Cli,
    scan_roots: &[std::path::PathBuf],
    query: &str,
    limit: Option<usize>,
    empty_mode: EmptyQueryMode,
    json: bool,
) -> i32 {
    let start = std::time::Instant::now();
    let roots: Vec<String> = scan_roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let daemon_resp = if cli.no_daemon {
        None
    } else {
        daemon_client::try_request(&Request::Search {
            roots: roots.clone(),
            query: query.to_string(),
            limit,
            empty_mode: Some(empty_mode),
        })
    };

    let (mode, matches): (&str, Vec<DesktopEntryOut>) = if let Some(resp) = daemon_resp {
        match resp {
            Response::Entries { entries } => ("daemon", entries),
            Response::Error { message } => {
                eprintln!("desktop-indexer: daemon error: {message} (fallback local)");
                local_search(scan_roots, query, limit, empty_mode)
            }
            _ => local_search(scan_roots, query, limit, empty_mode),
        }
    } else {
        local_search(scan_roots, query, limit, empty_mode)
    };

    trace(cli, &format!("mode={mode} (search)"));
    timing(mode, start);

    if json {
        print_json(&matches);
    } else {
        for e in &matches {
            println!("{}\t{}", e.id, e.name.as_deref().unwrap_or(""));
        }
    }

    0
}

fn local_search(
    scan_roots: &[std::path::PathBuf],
    query: &str,
    limit: Option<usize>,
    empty_mode: EmptyQueryMode,
) -> (&'static str, Vec<DesktopEntryOut>) {
    let result = scan_and_parse_desktop_files(scan_roots, None);
    let freqs = FrequencyStore::load();
    let lim = limit.unwrap_or(20);
    (
        "local",
        search_entries_with_usage_map_and_empty_mode(
            &result.entries,
            query,
            lim,
            freqs.map(),
            empty_mode,
        ),
    )
}
