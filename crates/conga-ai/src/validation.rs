//! Validation harness for candidate tactics (D49). A tactic clears
//! the gate iff the paired win rate vs its *unchanged* baseline has
//! Wilson lower bound > 0.5 at 95% confidence over
//! [`DEFAULT_PAIRS_PER_GATE`] paired deals. The vs-strong comparison
//! is reported but informational: a tactic can validly improve over
//! the baseline it wraps without closing the whole gap to the
//! strongest available agent.
//!
//! Reuses [`run_batch`] + [`report_from_pairs`] unchanged; the
//! "same seeds across gates" convention gives apples-to-apples for
//! the two paired comparisons.

use std::time::Instant;

use conga_core::Seat;
use conga_rules::StandardConga;

use crate::eval::{EvalReport, report_from_pairs, run_batch, wilson_ci};
use crate::player::Player;

/// Paired deals per gate — 1000 matches at Wilson ~±2 pp precision,
/// matching the m5 regression gate. Each `validate_tactic` call runs
/// two gates in sequence, so the real cost is ~2x this count of
/// paired deals.
pub const DEFAULT_PAIRS_PER_GATE: u32 = 500;

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub vs_unchanged: EvalReport,
    pub vs_strong: EvalReport,
    /// Pass gate: Wilson LB of the tactic's win rate against the
    /// unchanged baseline is strictly above 0.5 at 95%.
    pub passed: bool,
}

impl ValidationReport {
    pub fn vs_unchanged_wilson_lb(&self) -> f64 {
        wilson_ci(self.vs_unchanged.a_wins, self.vs_unchanged.games, 1.96).0
    }

    pub fn vs_strong_wilson_lb(&self) -> f64 {
        wilson_ci(self.vs_strong.a_wins, self.vs_strong.games, 1.96).0
    }
}

/// Run the validation loop: play `pairs` paired deals of `tactic` vs
/// `unchanged` and another `pairs` of `tactic` vs `strong`. Each
/// builder is called once per match (seats swapped between the two
/// matches of a pair), so cross-match state cannot leak. Both gates
/// start at the same `seed` and use consecutive seeds from there
/// for apples-to-apples comparison across the two gates.
pub fn validate_tactic<FT, FU, FS, PT, PU, PS>(
    mut tactic: FT,
    mut unchanged: FU,
    mut strong: FS,
    pairs: u32,
    seed: u64,
) -> ValidationReport
where
    FT: FnMut() -> PT,
    PT: Player,
    FU: FnMut() -> PU,
    PU: Player,
    FS: FnMut() -> PS,
    PS: Player,
{
    let rules = StandardConga::default();
    let dealer = Seat::new(0);

    let start_u = Instant::now();
    let pairs_vs_unchanged = run_batch(&rules, &mut tactic, &mut unchanged, pairs, seed, dealer);
    let vs_unchanged = report_from_pairs(&pairs_vs_unchanged, start_u.elapsed());

    let start_s = Instant::now();
    let pairs_vs_strong = run_batch(&rules, &mut tactic, &mut strong, pairs, seed, dealer);
    let vs_strong = report_from_pairs(&pairs_vs_strong, start_s.elapsed());

    let passed = wilson_ci(vs_unchanged.a_wins, vs_unchanged.games, 1.96).0 > 0.5;

    ValidationReport {
        vs_unchanged,
        vs_strong,
        passed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greedy::GreedyPlayer;
    use crate::random::RandomPlayer;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn dummy_good_tactic_passes_gate() {
        // Greedy as the tactic over Random as the unchanged baseline
        // is a well-known decisive edge — Wilson LB should clear 0.5
        // easily at 50 pairs.
        let seed = 0xC0FFEEu64;
        let report = validate_tactic(
            GreedyPlayer::new,
            || RandomPlayer::<ChaCha8Rng>::from_seed(seed),
            || RandomPlayer::<ChaCha8Rng>::from_seed(seed.wrapping_add(1)),
            50,
            seed,
        );
        assert!(
            report.passed,
            "greedy-vs-random: Wilson LB should clear 0.5, got {:.4}",
            report.vs_unchanged_wilson_lb()
        );
        assert!(report.vs_unchanged.a_wins > report.vs_unchanged.b_wins);
    }

    #[test]
    fn dummy_bad_tactic_fails_gate() {
        // Random as the tactic over Greedy as the unchanged baseline
        // should fail the Wilson LB > 0.5 check.
        let seed = 0xBAD_u64;
        let report = validate_tactic(
            || RandomPlayer::<ChaCha8Rng>::from_seed(seed),
            GreedyPlayer::new,
            GreedyPlayer::new,
            50,
            seed,
        );
        assert!(
            !report.passed,
            "random-vs-greedy: Wilson LB should stay at/below 0.5, got {:.4}",
            report.vs_unchanged_wilson_lb()
        );
    }

    #[test]
    fn report_shapes_match() {
        let seed = 7u64;
        let pairs = 20u32;
        let report = validate_tactic(
            GreedyPlayer::new,
            || RandomPlayer::<ChaCha8Rng>::from_seed(seed),
            || RandomPlayer::<ChaCha8Rng>::from_seed(seed),
            pairs,
            seed,
        );
        assert_eq!(report.vs_unchanged.games, 2 * pairs);
        assert_eq!(report.vs_strong.games, 2 * pairs);
    }
}
