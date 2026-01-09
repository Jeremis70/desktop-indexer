use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Search desktop entries (stub for now)
    Search {
        query: String,
        /// Max results to return (omit for unlimited)
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        json: bool,
    },

    /// List desktop entries (stub for now)
    List {
        #[arg(long)]
        json: bool,
    },

    /// Launch an app by desktop-id
    Launch {
        desktop_id: String,

        /// Optional Desktop Action id (from [Desktop Action <id>])
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

    /// Start IPC daemon (background)
    StartDaemon,

    /// Stop IPC daemon (ask it to shutdown)
    StopDaemon,

    /// Check daemon status
    Status {
        #[arg(long)]
        json: bool,
    },

    /// Internal: run daemon server (foreground)
    #[command(hide = true)]
    RunDaemon,
}
