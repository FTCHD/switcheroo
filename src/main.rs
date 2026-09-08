//! Switcheroo: one binary that switches the logged-in account of developer CLIs from the
//! terminal, a loopback web UI, or the system tray.

mod cli;
mod core;
mod providers;
mod server;
mod tray;
mod vault;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    std::process::exit(cli::run(cli));
}
