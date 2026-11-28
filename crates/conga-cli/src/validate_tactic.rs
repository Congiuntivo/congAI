//! `conga-cli validate-tactic` — runs the slice-4 validation loop on
//! a candidate tactic end-to-end: 500 paired deals (default) against
//! the unchanged baseline, another 500 against score-aware, prints
//! both reports and the Wilson CIs (D49).

use std::io::Write;
use std::time::Instant;

use conga_ai::candidates::CleanSplitCloseWrapper;
use conga_ai::validation::validate_tactic;
use conga_ai::{RuleBasedPlayer, ScoreAwarePlayer, wilson_ci};

#[derive(Debug)]
pub enum ValidateError {
    Io(std::io::Error),
    UnknownCandidate(String),
}

impl std::fmt::Display for ValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidateError::Io(e) => write!(f, "io: {e}"),
            ValidateError::UnknownCandidate(c) => write!(f, "unknown candidate: {c}"),
        }
    }
}

impl std::error::Error for ValidateError {}

impl From<std::io::Error> for ValidateError {
    fn from(value: std::io::Error) -> Self {
        ValidateError::Io(value)
    }
}

pub struct ValidateOptions {
    pub candidate: String,
    pub pairs: u32,
    pub seed: u64,
    pub samples: u32,
}

pub fn run<W: Write>(opts: ValidateOptions, mut out: W) -> Result<(), ValidateError> {
    match opts.candidate.as_str() {
        "clean-split-close" => run_clean_split_close(&opts, &mut out),
        other => Err(ValidateError::UnknownCandidate(other.to_string())),
    }
}

fn run_clean_split_close<W: Write>(
    opts: &ValidateOptions,
    out: &mut W,
) -> Result<(), ValidateError> {
    writeln!(
        out,
        "validate-tactic: candidate=clean-split-close pairs={} samples={} seed=0x{:X}",
        opts.pairs, opts.samples, opts.seed
    )?;
    let samples = opts.samples;
    let seed = opts.seed;

    let tactic = || CleanSplitCloseWrapper::new(RuleBasedPlayer::new(), samples, seed);
    let unchanged = RuleBasedPlayer::new;
    let strong = || ScoreAwarePlayer::new(samples, seed);

    let started = Instant::now();
    let report = validate_tactic(tactic, unchanged, strong, opts.pairs, opts.seed);
    let elapsed = started.elapsed();

    let (u_lo, u_hi) = wilson_ci(report.vs_unchanged.a_wins, report.vs_unchanged.games, 1.96);
    let (s_lo, s_hi) = wilson_ci(report.vs_strong.a_wins, report.vs_strong.games, 1.96);

    writeln!(
        out,
        "\n== vs unchanged (rule-based) ==\n  games={} a_wins={} b_wins={} draws={}",
        report.vs_unchanged.games,
        report.vs_unchanged.a_wins,
        report.vs_unchanged.b_wins,
        report.vs_unchanged.draws,
    )?;
    writeln!(
        out,
        "  win_rate={:.4}  wilson 95% [{:.4}, {:.4}]",
        report.vs_unchanged.a_win_rate(),
        u_lo,
        u_hi,
    )?;
    writeln!(
        out,
        "  mean_tactic_score={:.2}  mean_unchanged_score={:.2}  hands/match={:.2}  decisions/s={:.0}",
        report.vs_unchanged.mean_a_score,
        report.vs_unchanged.mean_b_score,
        report.vs_unchanged.hands_per_match,
        report.vs_unchanged.decisions_per_second,
    )?;

    writeln!(
        out,
        "\n== vs strong (score-aware) [informational] ==\n  games={} a_wins={} b_wins={} draws={}",
        report.vs_strong.games,
        report.vs_strong.a_wins,
        report.vs_strong.b_wins,
        report.vs_strong.draws,
    )?;
    writeln!(
        out,
        "  win_rate={:.4}  wilson 95% [{:.4}, {:.4}]",
        report.vs_strong.a_win_rate(),
        s_lo,
        s_hi,
    )?;
    writeln!(
        out,
        "  mean_tactic_score={:.2}  mean_strong_score={:.2}  hands/match={:.2}",
        report.vs_strong.mean_a_score,
        report.vs_strong.mean_b_score,
        report.vs_strong.hands_per_match,
    )?;

    writeln!(
        out,
        "\n== decision ==\n  passed={}  (Wilson LB vs unchanged: {:.4} > 0.5 ? {})  total wall clock: {:.1}s",
        report.passed,
        u_lo,
        u_lo > 0.5,
        elapsed.as_secs_f64(),
    )?;
    Ok(())
}
