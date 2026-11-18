//! `conga-cli eval` — run `games` matches between two baseline agents,
//! print a 95% Wilson confidence interval on A's win rate, mean scores
//! and timing. Headless and single-threaded.

use std::io::Write;
use std::time::Instant;

use conga_ai::{BaselineAgent, EvalReport, report_from_pairs, run_batch};
use conga_core::Seat;
use conga_rules::StandardConga;

/// AI-seed derivations so a single `--seed` reproduces the whole run.
/// Each agent gets its own stream so match-seed changes do not shift
/// the agent's RNG and vice versa.
const SEED_A_SALT: u64 = 0xA1A1_A1A1_A1A1_A1A1;
const SEED_B_SALT: u64 = 0xB2B2_B2B2_B2B2_B2B2;

#[derive(Debug)]
pub enum EvalError {
    UnknownAgent(String),
    Io(std::io::Error),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::UnknownAgent(name) => write!(f, "unknown agent: {name}"),
            EvalError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for EvalError {}

impl From<std::io::Error> for EvalError {
    fn from(value: std::io::Error) -> Self {
        EvalError::Io(value)
    }
}

pub struct EvalOptions {
    pub agent_a: String,
    pub agent_b: String,
    pub games: u32,
    pub seed: u64,
}

/// Run the eval and write a human-readable report to `out`. Returns
/// the computed `EvalReport` so tests can assert on numbers directly.
pub fn run<W: Write>(opts: EvalOptions, mut out: W) -> Result<EvalReport, EvalError> {
    validate_agent(&opts.agent_a)?;
    validate_agent(&opts.agent_b)?;

    let rules = StandardConga::default();
    let pairs = opts.games.div_ceil(2);
    let seed_a = opts.seed.wrapping_add(SEED_A_SALT);
    let seed_b = opts.seed.wrapping_add(SEED_B_SALT);
    let agent_a_name = opts.agent_a.clone();
    let agent_b_name = opts.agent_b.clone();

    let start = Instant::now();
    let results = run_batch(
        &rules,
        || BaselineAgent::from_name(&agent_a_name, seed_a).expect("validated above"),
        || BaselineAgent::from_name(&agent_b_name, seed_b).expect("validated above"),
        pairs,
        opts.seed,
        Seat::new(1),
    );
    let duration = start.elapsed();
    let report = report_from_pairs(&results, duration);
    write_report(&opts, &report, &mut out)?;
    Ok(report)
}

fn validate_agent(name: &str) -> Result<(), EvalError> {
    if BaselineAgent::names().contains(&name) {
        Ok(())
    } else {
        Err(EvalError::UnknownAgent(name.to_string()))
    }
}

fn write_report<W: Write>(opts: &EvalOptions, r: &EvalReport, out: &mut W) -> std::io::Result<()> {
    let (lo, hi) = r.a_win_rate_ci();
    let pairs = r.games / 2;
    writeln!(out, "Eval: {} vs {}", opts.agent_a, opts.agent_b)?;
    writeln!(
        out,
        "Games: {} ({} paired deals, seed 0x{:X})",
        r.games, pairs, opts.seed
    )?;
    writeln!(
        out,
        "{} wins: {} ({:.2}%, 95% Wilson CI [{:.2}%, {:.2}%])",
        opts.agent_a,
        r.a_wins,
        r.a_win_rate() * 100.0,
        lo * 100.0,
        hi * 100.0,
    )?;
    writeln!(
        out,
        "{} wins: {} ({:.2}%)",
        opts.agent_b,
        r.b_wins,
        if r.games > 0 {
            r.b_wins as f64 / r.games as f64 * 100.0
        } else {
            0.0
        },
    )?;
    if r.draws > 0 {
        writeln!(out, "Draws: {}", r.draws)?;
    }
    writeln!(
        out,
        "Mean final score: {} {:.1}  |  {} {:.1}",
        opts.agent_a, r.mean_a_score, opts.agent_b, r.mean_b_score,
    )?;
    writeln!(out, "Hands per match: {:.2}", r.hands_per_match)?;
    writeln!(out, "Decisions/sec: {:.0}", r.decisions_per_second)?;
    writeln!(
        out,
        "Duration: {}.{:03}s",
        r.duration_ms / 1000,
        r.duration_ms % 1000
    )?;
    Ok(())
}
