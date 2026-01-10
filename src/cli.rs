use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::empty_query::EmptyQueryMode;

#[derive(Subcommand, Debug)]
pub enum DaemonCmd {
    /// Start IPC daemon
    Start,
    /// Stop IPC daemon
    Stop,
    /// Restart IPC daemon (stop then start)
    Restart,
    /// Check daemon status
    Status {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Parser, Debug)]
#[command(name = "desktop-indexer")]
#[command(about = "Index/search .desktop files (WIP)", long_about = None)]
pub struct Cli {
    /// Extra scan roots (repeatable)
    #[arg(short = 'p', long = "path")]
    pub paths: Vec<PathBuf>,

    /// Print whether we used daemon or local fallback (stderr)
    #[arg(long, global = true)]
    pub trace: bool,

    /// Force local execution (do not use daemon)
    #[arg(long, global = true)]
    pub no_daemon: bool,

    /// If set, hide entries whose TryExec is present but not available.
    ///
    /// This matches common desktop launcher behavior: TryExec is a presence check,
    /// not an alternative Exec line.
    #[arg(long, global = true)]
    pub respect_try_exec: bool,

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Search desktop entries
    Search {
        query: String,
        /// Max results to return (omit for unlimited)
        #[arg(long)]
        limit: Option<usize>,

        /// When the query is empty/whitespace, return recent or frequent entries.
        #[arg(long, value_enum, default_value_t = EmptyQueryMode::Recency)]
        empty_mode: EmptyQueryMode,

        #[arg(long)]
        json: bool,
    },

    /// List desktop entries
    List {
        #[arg(long)]
        json: bool,
    },

    /// Launch an app by desktop-id
    Launch {
        desktop_id: String,

        /// Optional Desktop Action id
        #[arg(long)]
        action: Option<String>,
    },

    /// Scan for .desktop files and print what we found
    Scan {
        /// Max number of file paths to print (omit for unlimited)
        #[arg(long)]
        limit: Option<usize>,

        /// Parse each found .desktop file and output extracted fields
        #[arg(long)]
        parse: bool,

        /// Output JSON
        #[arg(long)]
        json: bool,
    },
    /// Parse a single .desktop file and print extracted fields
    Parse {
        path: PathBuf,

        #[arg(long)]
        json: bool,
    },

    /// Manage IPC daemon (start/stop/restart/status)
    Daemon {
        #[command(subcommand)]
        cmd: DaemonCmd,
    },

    /// Start IPC daemon
    StartDaemon,

    /// Stop IPC daemon
    StopDaemon,

    /// Check daemon status
    Status {
        #[arg(long)]
        json: bool,
    },

    /// Internal: run daemon server
    #[command(hide = true)]
    RunDaemon,
}
