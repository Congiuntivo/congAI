//! `conga-cli mine-disagreements` — writes a CSV of positions where
//! `ScoreAwarePlayer` and `RuleBasedPlayer` would pick different
//! actions (D46). Produces the committed diagnostic artefact at
//! `crates/conga-ai/diagnostics/m6_disagreements.csv`.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use conga_ai::disagreement::{mine_disagreements, write_csv};

#[derive(Debug)]
pub enum MineError {
    Io(std::io::Error),
}

impl std::fmt::Display for MineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MineError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for MineError {}

impl From<std::io::Error> for MineError {
    fn from(value: std::io::Error) -> Self {
        MineError::Io(value)
    }
}

pub struct MineOptions {
    pub seeds: u32,
    pub max_per_seed: u32,
    pub out: PathBuf,
}

pub fn run<W: Write>(opts: MineOptions, mut out: W) -> Result<(), MineError> {
    writeln!(
        out,
        "mine-disagreements: seeds=0..{} max_per_seed={} out={}",
        opts.seeds,
        opts.max_per_seed,
        opts.out.display(),
    )?;
    let start = Instant::now();
    let seed_list: Vec<u64> = (0..opts.seeds as u64).collect();
    let rows = mine_disagreements(&seed_list, opts.max_per_seed as usize);
    let elapsed = start.elapsed();

    let mut file = std::fs::File::create(&opts.out)?;
    write_csv(&rows, &mut file)?;

    writeln!(
        out,
        "wrote {} rows to {} ({:.1}s)",
        rows.len(),
        opts.out.display(),
        elapsed.as_secs_f64(),
    )?;
    Ok(())
}
