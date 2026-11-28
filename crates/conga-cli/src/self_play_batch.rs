//! `conga-cli self-play-batch` — generates the labelled dataset the
//! m7 value-model trainer consumes (D54). Plays paired self-play of
//! `score-aware` vs `score-aware` plus `clean-split-close` vs
//! `clean-split-close` and writes `(encoding, hand-end equity)` rows
//! to a binary file matching the `conga_ai::dataset` format.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use conga_ai::dataset::{generate, write_binary};

#[derive(Debug)]
pub enum SelfPlayBatchError {
    Io(std::io::Error),
}

impl std::fmt::Display for SelfPlayBatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelfPlayBatchError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for SelfPlayBatchError {}

impl From<std::io::Error> for SelfPlayBatchError {
    fn from(value: std::io::Error) -> Self {
        SelfPlayBatchError::Io(value)
    }
}

pub struct SelfPlayBatchOptions {
    pub matches_each: u32,
    pub seed: u64,
    pub out: PathBuf,
}

pub fn run<W: Write>(opts: SelfPlayBatchOptions, mut out: W) -> Result<(), SelfPlayBatchError> {
    writeln!(
        out,
        "self-play-batch: matches_each={} seed=0x{:X} out={}",
        opts.matches_each,
        opts.seed,
        opts.out.display(),
    )?;
    let start = Instant::now();
    let samples = generate(opts.matches_each, opts.seed);
    let gen_elapsed = start.elapsed();
    writeln!(
        out,
        "generated {} samples ({:.1}s, {:.0} samples/sec)",
        samples.len(),
        gen_elapsed.as_secs_f64(),
        samples.len() as f64 / gen_elapsed.as_secs_f64().max(1e-9),
    )?;

    write_binary(&opts.out, &samples)?;
    let total_bytes = std::fs::metadata(&opts.out).map(|m| m.len()).unwrap_or(0);
    writeln!(
        out,
        "wrote {} bytes ({:.1} MiB) to {} (total elapsed {:.1}s)",
        total_bytes,
        total_bytes as f64 / (1024.0 * 1024.0),
        opts.out.display(),
        start.elapsed().as_secs_f64(),
    )?;
    Ok(())
}
