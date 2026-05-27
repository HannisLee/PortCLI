mod cli;
mod config;
mod control;
mod daemon;
mod error;
mod forward;
mod logs;
mod state;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = cli::handle_command(cli) {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
