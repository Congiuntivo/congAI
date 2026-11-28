use std::io::IsTerminal;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use conga_cli::{
    build_equity, counterfactual_sweep, debug_hand, eval, mine_disagreements, play, replay,
    validate_tactic,
};

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
    /// Play a full match in the TUI against a baseline agent. Writes a
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
        /// Suit glyphs: `french` (default) uses ♣♦♥♠, `spanish` uses ✦●♡✚.
        #[arg(long, default_value = "french", value_parser = ["french", "spanish", "f", "s"])]
        suits: String,
        /// Opponent agent: `random`, `greedy`, `rule-based`, `search`, `score-aware` or `clean-split-close`.
        #[arg(long, default_value = "random", value_parser = ["random", "greedy", "rule-based", "search", "score-aware", "clean-split-close"])]
        ai: String,
        /// Belief-samples per decision for the `search` agent. Ignored
        /// by the other agents.
        #[arg(long, default_value_t = 50)]
        samples: u32,
    },
    /// Read a JSON-lines match log and print each hand's state
    /// sequence plus the final match status.
    Replay {
        /// Path to a JSON-lines match log written by `play`.
        log_file: PathBuf,
    },
    /// Build the 2-player match equity table by greedy-vs-greedy
    /// Monte Carlo (D40). Use `--smoke` to time 1000 matches at a
    /// mid-range cell and get an ETA for the full run before
    /// committing the artefact.
    BuildEquity {
        /// Matches simulated per cell in full mode.
        #[arg(long, default_value_t = 100)]
        games_per_cell: u32,
        /// Where the resulting equity JSON lands. Ignored under
        /// `--smoke`.
        #[arg(long)]
        out: PathBuf,
        /// Base RNG seed for the simulations.
        #[arg(long)]
        seed: u64,
        /// Measure matches/sec at one cell and print an ETA for the
        /// full build; do not write any file.
        #[arg(long, default_value_t = false)]
        smoke: bool,
    },
    /// Probe each position in a disagreements CSV for counterfactual
    /// sensitivity (score ±5/±10 on both sides, close-card swaps).
    /// Writes the aggregated sweep as JSON (D46).
    CounterfactualSweep {
        /// Disagreements CSV produced by `mine-disagreements`.
        #[arg(long)]
        from: PathBuf,
        /// Number of positions to probe (reads the top-N rows of the
        /// CSV).
        #[arg(long, default_value_t = 10)]
        n: u32,
        /// Belief-samples per equity lookup. Matches the score-aware
        /// agent's default (20).
        #[arg(long, default_value_t = 20)]
        samples: u32,
        /// Where to write the sweep JSON.
        #[arg(long)]
        out: PathBuf,
    },
    /// Mine positions where `score-aware` and `rule-based` disagree
    /// (D46). Writes a CSV to `--out`.
    MineDisagreements {
        /// Number of seeds to mine. Seeds run `0..seeds`.
        #[arg(long, default_value_t = 50)]
        seeds: u32,
        /// Cap on rows collected per seed; the match breaks early once
        /// hit.
        #[arg(long, default_value_t = 5)]
        max_per_seed: u32,
        /// Where to write the CSV.
        #[arg(long)]
        out: PathBuf,
    },
    /// Validate a candidate tactic through the m6 loop (D47, D49).
    /// Runs paired matches vs the unchanged baseline and vs the
    /// strong baseline, prints both reports and the pass decision.
    ValidateTactic {
        /// Candidate name. Current support: `clean-split-close`.
        #[arg(long, value_parser = ["clean-split-close"])]
        candidate: String,
        /// Paired deals per gate (so `2*pairs` total matches per
        /// gate). Default matches the m5 regression precision.
        #[arg(long, default_value_t = 500)]
        pairs: u32,
        /// Base seed for both gates' paired matches. Belief sampling
        /// inside the candidate derives from the same seed per match.
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// Belief samples per Close decision (candidate + strong).
        #[arg(long, default_value_t = 20)]
        samples: u32,
    },
    /// Head-to-head evaluation of two baseline agents.
    Eval {
        /// First agent: `random`, `greedy`, `rule-based`, `search`, `score-aware` or `clean-split-close`.
        #[arg(long = "a", value_parser = ["random", "greedy", "rule-based", "search", "score-aware", "clean-split-close"])]
        agent_a: String,
        /// Second agent.
        #[arg(long = "b", value_parser = ["random", "greedy", "rule-based", "search", "score-aware", "clean-split-close"])]
        agent_b: String,
        /// Number of matches to play (rounded up to the nearest even
        /// number so every chance stream is run with seats swapped).
        #[arg(long, default_value_t = 1000)]
        games: u32,
        /// Base RNG seed. The agents' RNGs are derived from this.
        #[arg(long)]
        seed: u64,
        /// Belief-samples per decision for the `search` agent. Ignored
        /// by the other agents.
        #[arg(long, default_value_t = 50)]
        samples: u32,
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
        Command::Play {
            seed,
            players,
            log,
            suits,
            ai,
            samples,
        } => {
            let theme = conga_tui::SuitTheme::parse(&suits).unwrap_or_default();
            let opts = play::PlayOptions {
                seed,
                players,
                log_path: log,
                theme,
                ai,
                samples,
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
        Command::BuildEquity {
            games_per_cell,
            out,
            seed,
            smoke,
        } => {
            let opts = build_equity::BuildEquityOptions {
                games_per_cell,
                out,
                seed,
                smoke,
            };
            let stdout = std::io::stdout().lock();
            if let Err(e) = build_equity::run(opts, stdout) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Command::CounterfactualSweep {
            from,
            n,
            samples,
            out,
        } => {
            let opts = counterfactual_sweep::SweepOptions {
                from,
                n,
                samples,
                out,
            };
            let stdout = std::io::stdout().lock();
            if let Err(e) = counterfactual_sweep::run(opts, stdout) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Command::MineDisagreements {
            seeds,
            max_per_seed,
            out,
        } => {
            let opts = mine_disagreements::MineOptions {
                seeds,
                max_per_seed,
                out,
            };
            let stdout = std::io::stdout().lock();
            if let Err(e) = mine_disagreements::run(opts, stdout) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Command::ValidateTactic {
            candidate,
            pairs,
            seed,
            samples,
        } => {
            let opts = validate_tactic::ValidateOptions {
                candidate,
                pairs,
                seed,
                samples,
            };
            let stdout = std::io::stdout().lock();
            if let Err(e) = validate_tactic::run(opts, stdout) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Command::Eval {
            agent_a,
            agent_b,
            games,
            seed,
            samples,
        } => {
            let opts = eval::EvalOptions {
                agent_a,
                agent_b,
                games,
                seed,
                samples,
            };
            let stdout = std::io::stdout().lock();
            if let Err(e) = eval::run(opts, stdout) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}
