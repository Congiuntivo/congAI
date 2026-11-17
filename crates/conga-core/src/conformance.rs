//! Property checks every rule set must pass before anything else uses it.
//!
//! The checks are:
//!
//! 1. All cards of the deck are accounted for after every action.
//! 2. No observation contains a card its seat should not see.
//! 3. A hand that is still running always has a legal action.
//! 4. A logged game replays to identical states.
//! 5. Every sampled state reproduces the observation it was sampled from.
//!
//! Checks 1 to 4 arrive in slice 5 alongside the first `RuleSet`
//! implementation (`StandardConga`). Check 5 depends on `sample_state`, which
//! is a milestone-4 concern.

use crate::ruleset::RuleSet;

/// Play `games_per_seed` random games with each seed in `seeds`, running
/// property checks 1 to 4 after every action.
///
/// This function is defined now so the trait, the invariants list and the
/// `#[cfg(test)]` wiring stay in one module. The body lands in slice 5 when
/// the first `RuleSet` implementation makes it exercisable.
pub fn run_random_games<R: RuleSet>(_rules: &R, _seeds: &[u64], _games_per_seed: u32) {
    unimplemented!("conformance tests activate in slice 5 with StandardConga")
}
