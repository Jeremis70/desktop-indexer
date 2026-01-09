mod app;
mod cache;
mod cli;
mod commands;
mod daemon;
mod daemon_client;
mod desktop;
mod empty_query;
mod frequency;
mod ipc;
mod launch;
mod models;
mod output;
mod search;
mod xdg;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    let code = app::run(cli);
    if code != 0 {
        std::process::exit(code);
    }
}
