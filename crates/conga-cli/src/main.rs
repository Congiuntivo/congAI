use std::io::IsTerminal;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use conga_cli::{debug_hand, play, replay};

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
    /// Play a full match in the TUI against the random agent. Writes a
    /// JSON-lines log that `replay` can read back.
    Play {
        /// RNG seed for the match's shuffles and the AI's choices.
        #[arg(long)]
        seed: u64,
        /// Number of players. m2 only supports 2.
        #[arg(long, default_value_t = 2)]
        players: u8,
        /// Where to write the JSON-lines match log.
        #[arg(long)]
        log: PathBuf,
    },
    /// Read a JSON-lines match log and print each hand's state
    /// sequence plus the final match status.
    Replay {
        /// Path to a JSON-lines match log written by `play`.
        log_file: PathBuf,
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
        Command::Play { seed, players, log } => {
            let opts = play::PlayOptions {
                seed,
                players,
                log_path: log,
            };
            match play::run(opts) {
                Ok((status, scores)) => {
                    // Printed after the TUI has torn down its raw mode.
                    println!("match end: {status:?}  scores: {scores:?}");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Command::Replay { log_file } => {
            let opts = replay::ReplayOptions { log_path: log_file };
            let stdout = std::io::stdout().lock();
            match replay::run(opts, stdout) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}
