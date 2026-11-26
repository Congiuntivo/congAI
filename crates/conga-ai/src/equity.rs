//! 2-player match equity table (D39).
//!
//! Maps a pair of running match scores `(own, opp)` to the win
//! probability for the `own` seat under a fixed baseline (greedy vs
//! greedy, see D40). Dealer-agnostic: the dealer alternates each hand
//! in 2-player play, so any dealer bias averages out.
//!
//! Storage is a dense row-major `Vec<f32>` of
//! `(MAX_SCORE - MIN_SCORE + 1)^2 = 161 * 161 = 25_921` cells, indexed
//! by `(own - MIN_SCORE, opp - MIN_SCORE)`. Scores outside the stored
//! range are clamped to the nearest edge. Elimination short-circuits
//! the lookup before any clamping: `own > ELIMINATION_THRESHOLD`
//! returns `0.0`, and `opp > ELIMINATION_THRESHOLD` (with `own` not
//! itself eliminated) returns `1.0`.
//!
//! Slice 1 ships only the *placeholder* table (an analytical
//! approximation to the win probability) so slices 4 and 5 can run
//! end-to-end before the real Monte Carlo compute in slice 3 lands.

use serde::{Deserialize, Serialize};

/// Inclusive lower score stored in the table.
pub const MIN_SCORE: i32 = -50;

/// Inclusive upper score stored in the table.
pub const MAX_SCORE: i32 = 110;

/// A score strictly greater than this means the seat is eliminated
/// from the match. Lookups short-circuit on either side being over.
pub const ELIMINATION_THRESHOLD: i32 = 100;

/// Side length of the stored grid (161 for the default bounds).
pub const DIM: usize = (MAX_SCORE - MIN_SCORE + 1) as usize;

/// A 2-player match equity table. Serializes as JSON with the three
/// fields `min_score`, `max_score`, `values` so the committed artefact
/// is human-inspectable and diffable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquityTable {
    pub min_score: i32,
    pub max_score: i32,
    pub values: Vec<f32>,
}

impl EquityTable {
    /// Length of one side of the square grid.
    pub fn dim(&self) -> usize {
        (self.max_score - self.min_score + 1) as usize
    }

    fn index(&self, own: i32, opp: i32) -> usize {
        let dim = self.dim();
        let o = (own.clamp(self.min_score, self.max_score) - self.min_score) as usize;
        let p = (opp.clamp(self.min_score, self.max_score) - self.min_score) as usize;
        o * dim + p
    }

    /// Win probability for the `own` seat at score `(own, opp)`.
    pub fn lookup(&self, own: i32, opp: i32) -> f32 {
        if own > ELIMINATION_THRESHOLD {
            return 0.0;
        }
        if opp > ELIMINATION_THRESHOLD {
            return 1.0;
        }
        self.values[self.index(own, opp)]
    }

    /// Analytical placeholder: equity is the fraction of remaining
    /// "headroom" to elimination (101 - score). Satisfies
    /// `placeholder().lookup(a, b) + placeholder().lookup(b, a) == 1`
    /// exactly, and `placeholder().lookup(0, 0) == 0.5`. Cells where
    /// either side is past the elimination threshold are stored as
    /// 0.0 / 1.0 to match `lookup`'s short-circuit (and to keep the
    /// formula free of the 0/0 singularity it would otherwise hit
    /// when `a + b == 202`). Replaced by slice-3 Monte Carlo values.
    pub fn placeholder() -> Self {
        let mut values = Vec::with_capacity(DIM * DIM);
        for a in MIN_SCORE..=MAX_SCORE {
            for b in MIN_SCORE..=MAX_SCORE {
                let v = if a > ELIMINATION_THRESHOLD {
                    0.0
                } else if b > ELIMINATION_THRESHOLD {
                    1.0
                } else {
                    let num = (101 - a) as f32;
                    let den = num + (101 - b) as f32;
                    num / den
                };
                values.push(v);
            }
        }
        EquityTable {
            min_score: MIN_SCORE,
            max_score: MAX_SCORE,
            values,
        }
    }

    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn to_json_string_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Path to the committed artefact, resolved at compile time relative
/// to this crate's manifest. `include_str!` would bake the file into
/// the binary and force a recompile whenever the table changes, which
/// is unhelpful while slice 2's `build-equity` CLI iterates; reading
/// at runtime keeps the regen loop fast. The path is absolute so it
/// works regardless of the caller's cwd.
pub const COMMITTED_ASSET_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/equity_2p.json");

/// The committed equity artefact, parsed at first access.
pub fn committed_table() -> &'static EquityTable {
    use std::sync::OnceLock;
    static TABLE: OnceLock<EquityTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let s = std::fs::read_to_string(COMMITTED_ASSET_PATH)
            .expect("committed equity_2p.json must exist");
        EquityTable::from_json_str(&s).expect("committed equity_2p.json must parse")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_half_at_origin() {
        let t = EquityTable::placeholder();
        assert!((t.lookup(0, 0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn placeholder_mirror_symmetry() {
        let t = EquityTable::placeholder();
        for (a, b) in [(-50, 100), (-10, 20), (5, 5), (0, 73), (100, -45)] {
            let sum = t.lookup(a, b) + t.lookup(b, a);
            assert!(
                (sum - 1.0).abs() < 1e-6,
                "mirror failed at ({a},{b}): {} + {} = {sum}",
                t.lookup(a, b),
                t.lookup(b, a),
            );
        }
    }

    #[test]
    fn placeholder_monotone_in_own_score() {
        let t = EquityTable::placeholder();
        for b in [-30, 0, 40, 80] {
            let mut prev = f32::INFINITY;
            for a in (MIN_SCORE..=ELIMINATION_THRESHOLD).step_by(5) {
                let v = t.lookup(a, b);
                assert!(
                    v <= prev,
                    "monotonicity broke at a={a}, b={b}: {v} > {prev}"
                );
                prev = v;
            }
        }
    }

    #[test]
    fn lookup_clamps_below_floor() {
        let t = EquityTable::placeholder();
        assert_eq!(t.lookup(-200, -200), t.lookup(-50, -50));
        assert_eq!(t.lookup(-100, 50), t.lookup(-50, 50));
    }

    #[test]
    fn eliminated_own_is_zero() {
        let t = EquityTable::placeholder();
        assert_eq!(t.lookup(101, 50), 0.0);
        assert_eq!(t.lookup(200, -30), 0.0);
    }

    #[test]
    fn eliminated_opp_is_one() {
        let t = EquityTable::placeholder();
        assert_eq!(t.lookup(50, 101), 1.0);
        assert_eq!(t.lookup(-30, 200), 1.0);
    }

    #[test]
    fn both_eliminated_prefers_own_loss() {
        // If both scores are over the elimination threshold, the own
        // seat is eliminated first by the short-circuit. This is a
        // degenerate state (it should never arise in real play; a hand
        // ends when the first seat crosses) but the lookup is defined.
        let t = EquityTable::placeholder();
        assert_eq!(t.lookup(105, 102), 0.0);
    }

    #[test]
    fn serde_roundtrip_f32_bit_identical() {
        let t = EquityTable::placeholder();
        let json = t.to_json_string_pretty().unwrap();
        let back = EquityTable::from_json_str(&json).unwrap();
        assert_eq!(t.min_score, back.min_score);
        assert_eq!(t.max_score, back.max_score);
        assert_eq!(t.values.len(), back.values.len());
        for (i, (a, b)) in t.values.iter().zip(back.values.iter()).enumerate() {
            assert_eq!(a.to_bits(), b.to_bits(), "cell {i} not bit-identical");
        }
    }

    #[test]
    fn committed_artefact_loads() {
        let t = committed_table();
        assert_eq!(t.min_score, MIN_SCORE);
        assert_eq!(t.max_score, MAX_SCORE);
        assert_eq!(t.values.len(), DIM * DIM);
    }

    /// The committed table is a Monte Carlo result (greedy vs greedy,
    /// N samples per cell, see D40). Noise bounds are generous enough
    /// to pass at N=20 and tighten at N=50+.
    #[test]
    fn real_origin_near_half() {
        let t = committed_table();
        let v = t.lookup(0, 0);
        assert!(
            (v - 0.5).abs() <= 0.08,
            "committed equity at (0,0) is {v}, expected ~0.5",
        );
    }

    #[test]
    fn real_eliminated_boundary() {
        let t = committed_table();
        assert_eq!(t.lookup(101, 0), 0.0, "own eliminated must be 0");
        assert_eq!(t.lookup(0, 101), 1.0, "opp eliminated must be 1");
        assert!(
            t.lookup(-50, 0) > 0.6,
            "deep negative own score should dominate (got {})",
            t.lookup(-50, 0)
        );
        assert!(
            t.lookup(0, -50) < 0.4,
            "deep negative opp score should lose (got {})",
            t.lookup(0, -50)
        );
    }

    #[test]
    fn real_mirror_symmetry_off_diagonal() {
        let t = committed_table();
        // Off-diagonal mirror is baked in by the compute (slice 2
        // writes `values[idx_ba] = 1 - values[idx_ab]`), so the
        // invariant is exact up to the f32 rounding of (1.0 - v).
        // Diagonal cells (a == b) are *not* mirror-pinned because
        // mirror would double-count the single Monte Carlo estimate;
        // they float around 0.5 within sampling noise and are covered
        // by `real_diagonal_near_half`.
        for (a, b) in [(-40, 60), (-10, 30), (25, 70), (90, -20), (-50, 100)] {
            assert_ne!(a, b, "off-diagonal test used a diagonal cell");
            let sum = t.lookup(a, b) + t.lookup(b, a);
            assert!(
                (sum - 1.0).abs() < 1e-6,
                "mirror broken at ({a},{b}): {} + {} = {sum}",
                t.lookup(a, b),
                t.lookup(b, a),
            );
        }
    }

    #[test]
    fn real_diagonal_near_half() {
        // Equal scores → equity ≈ 0.5 by greedy-vs-greedy symmetry,
        // modulo Monte Carlo noise at N=50 (std ≈ 0.071). Use a wide
        // tolerance: ~2 sigma + safety margin.
        let t = committed_table();
        for a in [-40, -10, 0, 25, 70] {
            let v = t.lookup(a, a);
            assert!(
                (v - 0.5).abs() <= 0.15,
                "diagonal ({a},{a}) is {v}, expected ~0.5",
            );
        }
    }

    #[test]
    fn real_monotone_in_own_within_noise() {
        // Equity must not increase as own score rises (holding opp
        // fixed). Tolerate up to `ulp` of Monte Carlo noise per step.
        let t = committed_table();
        let ulp = 0.15;
        for b in [-20, 0, 40, 80] {
            let mut prev = f32::INFINITY;
            for a in (MIN_SCORE..=ELIMINATION_THRESHOLD).step_by(10) {
                let v = t.lookup(a, b);
                assert!(
                    v <= prev + ulp,
                    "monotonicity broken at a={a}, b={b}: {v} > {prev} + {ulp}",
                );
                prev = v;
            }
        }
    }
}
