//! Regression guard for D31 (stalemate terminal). Two deterministic
//! greedy agents must now terminate every hand and every match: before
//! the stalemate fix these matches hung forever because `Close(c)` was
//! never offered by the engine. Tight budgets catch any future change
//! that re-opens the loop.

use conga_ai::{GreedyPlayer, RuleBasedPlayer, run_match};
use conga_core::Seat;
use conga_rules::StandardConga;

#[test]
fn greedy_vs_greedy_terminates_within_budget() {
    let rules = StandardConga::default();
    // 20 matches across a range of seeds. Each hand should take well
    // under 200 turns (3 reshuffles + residual draws) and each match
    // well under 50 hands.
    for seed in 0u64..20 {
        let mut a = GreedyPlayer::new();
        let mut b = GreedyPlayer::new();
        let outcome = run_match(&rules, &mut a, &mut b, Seat::new(0), seed);
        assert!(
            outcome.hands_played < 50,
            "seed {seed}: {} hands (expected < 50)",
            outcome.hands_played
        );
        // decisions includes both agents across every hand; a 50-hand
        // cap × 200 turns × 2 seats = 20_000 actions as a very loose
        // sanity upper bound.
        assert!(
            outcome.decisions < 20_000,
            "seed {seed}: {} decisions (expected < 20000)",
            outcome.decisions
        );
    }
}

#[test]
fn rule_based_vs_greedy_terminates_within_budget() {
    let rules = StandardConga::default();
    for seed in 0u64..20 {
        let mut a = RuleBasedPlayer::new();
        let mut b = GreedyPlayer::new();
        let outcome = run_match(&rules, &mut a, &mut b, Seat::new(0), seed);
        assert!(
            outcome.hands_played < 50,
            "seed {seed}: {} hands",
            outcome.hands_played
        );
        assert!(
            outcome.decisions < 20_000,
            "seed {seed}: {} decisions",
            outcome.decisions
        );
    }
}
