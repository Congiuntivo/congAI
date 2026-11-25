//! `conga-cli build-equity` — Monte Carlo the 2-player match equity
//! table by running greedy-vs-greedy matches starting from every
//! `(own, opp)` score pair (D40). Only cells with `own <= opp` are
//! simulated; the opposite half is filled via the mirror
//! `equity(a, b) = 1 - equity(b, a)` for free. Cells where either
//! side is past the elimination threshold are pre-filled with 0.0 or
//! 1.0 to match `EquityTable::lookup`'s short-circuit.
//!
//! `--smoke` samples 1000 matches at a mid-range cell to measure
//! matches/sec on this machine and prints an ETA for the full run
//! without touching the committed artefact. Slice 2 stops here; slice
//! 3 runs the full budget once Simone approves the numbers.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use conga_ai::equity::{DIM, ELIMINATION_THRESHOLD, EquityTable, MAX_SCORE, MIN_SCORE};
use conga_ai::{GreedyPlayer, run_match_from};
use conga_core::Seat;
use conga_rules::StandardConga;
use conga_server::MatchState;

/// Seed offsets that keep each cell's trajectories independent from
/// each other and from seed offsets used by `conga-cli eval`.
const CELL_STRIDE: u64 = 0x9E37_79B9_7F4A_7C15;
const ITER_STRIDE: u64 = 0x5A51_7A73_2A3B_1C4D;

#[derive(Debug)]
pub enum BuildEquityError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for BuildEquityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildEquityError::Io(e) => write!(f, "io: {e}"),
            BuildEquityError::Json(e) => write!(f, "json: {e}"),
        }
    }
}

impl std::error::Error for BuildEquityError {}

impl From<std::io::Error> for BuildEquityError {
    fn from(value: std::io::Error) -> Self {
        BuildEquityError::Io(value)
    }
}

impl From<serde_json::Error> for BuildEquityError {
    fn from(value: serde_json::Error) -> Self {
        BuildEquityError::Json(value)
    }
}

pub struct BuildEquityOptions {
    pub games_per_cell: u32,
    pub out: PathBuf,
    pub seed: u64,
    pub smoke: bool,
}

/// Win fraction for seat 0 from greedy-vs-greedy matches that start
/// with scores `(own, opp)` on seats `(0, 1)`. Cells past elimination
/// short-circuit without any simulation. Dealer alternates across the
/// `n` trials so any dealer bias averages out (D39).
pub fn compute_cell(own: i32, opp: i32, n: u32, base_seed: u64) -> f32 {
    if own > ELIMINATION_THRESHOLD {
        return 0.0;
    }
    if opp > ELIMINATION_THRESHOLD {
        return 1.0;
    }
    if n == 0 {
        return 0.5;
    }
    let rules = StandardConga::default();
    let cell_idx = ((own - MIN_SCORE) as u64) * DIM as u64 + ((opp - MIN_SCORE) as u64);
    let cell_seed = base_seed.wrapping_add(cell_idx.wrapping_mul(CELL_STRIDE));
    let mut wins = 0u32;
    for i in 0..n {
        let dealer = Seat::new((i % 2) as u8);
        let mut ms = MatchState::new(2, dealer);
        ms.scores[0] = own;
        ms.scores[1] = opp;
        let seed = cell_seed.wrapping_add((i as u64).wrapping_mul(ITER_STRIDE));
        let mut a = GreedyPlayer::new();
        let mut b = GreedyPlayer::new();
        let outcome = run_match_from(&rules, &mut a, &mut b, ms, seed);
        if outcome.winner_seat == Some(Seat::new(0)) {
            wins += 1;
        }
    }
    wins as f32 / n as f32
}

/// Simulate the upper triangle + diagonal (`own <= opp <= 100`), mirror
/// into the lower triangle, and pre-fill eliminated cells. Returns the
/// finished table alongside the number of matches actually simulated
/// (for the progress / timing report).
pub fn compute_table<F: FnMut(usize, usize)>(
    n: u32,
    base_seed: u64,
    mut progress: F,
) -> (EquityTable, u64) {
    let mut values = vec![0.5f32; DIM * DIM];

    // Pre-fill eliminated cells to match `EquityTable::lookup` semantics.
    for a in MIN_SCORE..=MAX_SCORE {
        for b in MIN_SCORE..=MAX_SCORE {
            let idx = cell_index(a, b);
            if a > ELIMINATION_THRESHOLD {
                values[idx] = 0.0;
            } else if b > ELIMINATION_THRESHOLD {
                values[idx] = 1.0;
            }
        }
    }

    let simulated_cells = upper_triangle_cell_count();
    let mut cells_done = 0usize;
    let mut matches_simulated = 0u64;
    for a in MIN_SCORE..=ELIMINATION_THRESHOLD {
        for b in a..=ELIMINATION_THRESHOLD {
            let v = compute_cell(a, b, n, base_seed);
            let idx_ab = cell_index(a, b);
            values[idx_ab] = v;
            if a != b {
                let idx_ba = cell_index(b, a);
                values[idx_ba] = 1.0 - v;
            }
            matches_simulated += n as u64;
            cells_done += 1;
            progress(cells_done, simulated_cells);
        }
    }

    let table = EquityTable {
        min_score: MIN_SCORE,
        max_score: MAX_SCORE,
        values,
    };
    (table, matches_simulated)
}

/// Number of cells that are actually Monte-Carlo-simulated (upper
/// triangle + diagonal, neither side eliminated). The remainder are
/// either filled by mirror symmetry or by the elimination short-circuit.
pub fn upper_triangle_cell_count() -> usize {
    let live = (ELIMINATION_THRESHOLD - MIN_SCORE + 1) as usize;
    live * (live + 1) / 2
}

fn cell_index(own: i32, opp: i32) -> usize {
    ((own - MIN_SCORE) as usize) * DIM + ((opp - MIN_SCORE) as usize)
}

/// Entrypoint. In smoke mode, runs 1000 matches at a mid-range cell
/// and reports matches/sec + an estimated full-run wall clock without
/// writing to the committed artefact. In full mode, runs the whole
/// table and writes the JSON to `opts.out`.
pub fn run<W: Write>(opts: BuildEquityOptions, mut out: W) -> Result<(), BuildEquityError> {
    if opts.smoke {
        run_smoke(opts, &mut out)
    } else {
        run_full(opts, &mut out)
    }
}

const SMOKE_MATCHES: u32 = 1000;
const SMOKE_CELL_OWN: i32 = 20;
const SMOKE_CELL_OPP: i32 = 60;

fn run_smoke<W: Write>(opts: BuildEquityOptions, out: &mut W) -> Result<(), BuildEquityError> {
    writeln!(
        out,
        "build-equity smoke: {SMOKE_MATCHES} greedy-vs-greedy matches at cell \
         (own={SMOKE_CELL_OWN}, opp={SMOKE_CELL_OPP}), seed 0x{:X}",
        opts.seed
    )?;
    let start = Instant::now();
    let p = compute_cell(SMOKE_CELL_OWN, SMOKE_CELL_OPP, SMOKE_MATCHES, opts.seed);
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64().max(1e-9);
    let matches_per_sec = SMOKE_MATCHES as f64 / secs;
    let cells = upper_triangle_cell_count() as f64;
    let n_full = opts.games_per_cell.max(1) as f64;
    let est_secs_full = cells * n_full / matches_per_sec;

    writeln!(
        out,
        "seat-0 win fraction at (own={SMOKE_CELL_OWN}, opp={SMOKE_CELL_OPP}): {p:.4}"
    )?;
    writeln!(
        out,
        "elapsed: {:.3}s  matches/sec: {:.1}",
        secs, matches_per_sec
    )?;
    writeln!(
        out,
        "full-run estimate at N={}: {} cells x {} matches = {:.0} matches, ~{:.0}s ({:.1} min)",
        opts.games_per_cell,
        cells as u64,
        opts.games_per_cell,
        cells * n_full,
        est_secs_full,
        est_secs_full / 60.0,
    )?;
    writeln!(out, "no file written (--smoke).")?;
    Ok(())
}

fn run_full<W: Write>(opts: BuildEquityOptions, out: &mut W) -> Result<(), BuildEquityError> {
    writeln!(
        out,
        "build-equity: N={} per cell, seed 0x{:X}, writing {}",
        opts.games_per_cell,
        opts.seed,
        opts.out.display()
    )?;
    let start = Instant::now();
    let total_cells = upper_triangle_cell_count();
    let mut last_report = 0usize;
    let report_every = (total_cells / 20).max(1);
    let (table, matches) = compute_table(opts.games_per_cell, opts.seed, |done, total| {
        if done - last_report >= report_every || done == total {
            last_report = done;
            let elapsed = start.elapsed().as_secs_f64();
            let eta = if done > 0 {
                elapsed * (total - done) as f64 / done as f64
            } else {
                0.0
            };
            // Progress goes to stderr so the stdout is just the status
            // report at the end. Uses a plain println so it works when
            // `out` is a buffer.
            eprintln!(
                "  {done}/{total} cells  ({:.1}%)  elapsed={:.1}s  eta={:.1}s",
                done as f64 * 100.0 / total as f64,
                elapsed,
                eta,
            );
        }
    });
    let elapsed = start.elapsed();
    let json = table.to_json_string_pretty()?;
    std::fs::write(&opts.out, json)?;
    writeln!(
        out,
        "wrote {} cells to {} ({} simulated matches, {:.1}s, {:.0} matches/sec)",
        table.values.len(),
        opts.out.display(),
        matches,
        elapsed.as_secs_f64(),
        matches as f64 / elapsed.as_secs_f64().max(1e-9),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eliminated_own_short_circuits_without_simulation() {
        // n=0 would normally return 0.5; the elimination short-circuit
        // must beat it.
        assert_eq!(compute_cell(101, 50, 0, 1), 0.0);
        assert_eq!(compute_cell(101, 50, 100, 1), 0.0);
        assert_eq!(compute_cell(200, -30, 100, 1), 0.0);
    }

    #[test]
    fn eliminated_opp_short_circuits_without_simulation() {
        assert_eq!(compute_cell(50, 101, 0, 1), 1.0);
        assert_eq!(compute_cell(50, 101, 100, 1), 1.0);
    }

    #[test]
    fn compute_cell_tiny_values_in_unit_interval() {
        for &(a, b) in &[(-50, -50), (-45, 0), (0, 0), (50, 50), (100, 100)] {
            let v = compute_cell(a, b, 5, 7);
            assert!(
                (0.0..=1.0).contains(&v),
                "value out of range at ({a},{b}): {v}"
            );
        }
    }

    #[test]
    fn compute_cell_deterministic_in_seed() {
        let a = compute_cell(10, 40, 8, 42);
        let b = compute_cell(10, 40, 8, 42);
        assert_eq!(a, b);
    }

    #[test]
    fn upper_triangle_covers_live_half() {
        // 151 live scores per side (-50..=100). Upper triangle plus
        // diagonal = 151 * 152 / 2 = 11_476.
        assert_eq!(upper_triangle_cell_count(), 151 * 152 / 2);
    }

    #[test]
    fn compute_cell_pair_mirrors_within_sampling_noise() {
        // The mirror fill in `compute_table` assumes
        // `compute_cell(a, b) + compute_cell(b, a) ~= 1` under
        // greedy-vs-greedy. Test that at one off-diagonal pair with
        // tiny N; wide tolerance because 10 samples is noisy. A
        // tighter version at larger N could be added as `#[ignore]`.
        let (a, b) = (10, 60);
        let n = 10;
        let v_ab = compute_cell(a, b, n, 7);
        let v_ba = compute_cell(b, a, n, 7);
        assert!(
            (v_ab + v_ba - 1.0).abs() < 0.4,
            "mirror broken beyond noise at ({a},{b}): {v_ab} + {v_ba}",
        );
    }
}
