use crate::cli::Cli;

pub fn timing_enabled() -> bool {
    matches!(
        std::env::var("DESKTOP_INDEXER_TIMING").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

pub fn trace(cli: &Cli, msg: &str) {
    if cli.trace {
        eprintln!("desktop-indexer: {msg}");
    }
}

pub fn timing(mode: &str, start: std::time::Instant) {
    if timing_enabled() {
        eprintln!(
            "desktop-indexer timing(client): mode={mode} elapsed={:?}",
            start.elapsed()
        );
    }
}
