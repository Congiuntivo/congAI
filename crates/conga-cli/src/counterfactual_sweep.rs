//! `conga-cli counterfactual-sweep` — reads the top-N rows from a
//! disagreements CSV, probes each position for counterfactual
//! sensitivity (D46), and writes the aggregated [`CounterfactualSweep`]
//! as pretty JSON. Produces the committed diagnostic at
//! `crates/conga-ai/diagnostics/m6_counterfactuals.json`.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use conga_ai::counterfactual::{ProbeTarget, sweep, write_json};
use conga_ai::disagreement::read_csv_rows;
use conga_core::Phase;

#[derive(Debug)]
pub enum SweepError {
    Io(std::io::Error),
}

impl std::fmt::Display for SweepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SweepError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for SweepError {}

impl From<std::io::Error> for SweepError {
    fn from(value: std::io::Error) -> Self {
        SweepError::Io(value)
    }
}

pub struct SweepOptions {
    pub from: PathBuf,
    pub n: u32,
    pub samples: u32,
    pub out: PathBuf,
}

pub fn run<W: Write>(opts: SweepOptions, mut out: W) -> Result<(), SweepError> {
    writeln!(
        out,
        "counterfactual-sweep: from={} n={} samples={} out={}",
        opts.from.display(),
        opts.n,
        opts.samples,
        opts.out.display(),
    )?;
    let file = std::fs::File::open(&opts.from)?;
    let records = read_csv_rows(file)?;
    let targets: Vec<ProbeTarget> = records
        .iter()
        .take(opts.n as usize)
        .filter_map(|r| {
            let phase = parse_phase(&r.phase)?;
            Some(ProbeTarget {
                seed: r.seed,
                hand: r.hand,
                turn: r.turn,
                current_seat: r.current_seat,
                phase,
            })
        })
        .collect();

    let start = Instant::now();
    let result = sweep(&targets, opts.samples);
    let elapsed = start.elapsed();

    let mut file = std::fs::File::create(&opts.out)?;
    write_json(&result, &mut file)?;

    writeln!(
        out,
        "wrote {} probe rows from {} positions ({} missing) in {:.1}s to {}",
        result.rows.len(),
        result.positions_probed,
        result.positions_missing.len(),
        elapsed.as_secs_f64(),
        opts.out.display(),
    )?;
    Ok(())
}

fn parse_phase(s: &str) -> Option<Phase> {
    match s {
        "Draw" => Some(Phase::Draw),
        "Discard" => Some(Phase::Discard),
        "Showdown" => Some(Phase::Showdown),
        "Ended" => Some(Phase::Ended),
        _ => None,
    }
}
