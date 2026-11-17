use std::io::IsTerminal;

use clap::{Parser, Subcommand};

mod debug_hand;

#[derive(Parser)]
#[command(name = "conga-cli", about = "Conga engine command-line tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Play one hand of Conga in the terminal, with every seat's hand
    /// visible. Useful for checking the rules by hand before the TUI
    /// exists.
    DebugHand {
        /// RNG seed. Same seed reproduces the same shuffle.
        #[arg(long)]
        seed: u64,
        /// Number of seats (2..=4).
        #[arg(long, default_value_t = 2)]
        players: u8,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::DebugHand { seed, players } => {
            let use_color = std::io::stdout().is_terminal();
            let stdin = std::io::stdin().lock();
            let stdout = std::io::stdout().lock();
            let opts = debug_hand::Options {
                seed,
                players,
                use_color,
            };
            if let Err(e) = debug_hand::run(opts, stdin, stdout) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}
