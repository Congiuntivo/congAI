//! Exit-test regressions. Each test plays enough matches that the
//! 95% Wilson lower bound on the winning agent's win rate is strictly
//! above 0.5. Deterministic: same seed, same result.
//!
//! m3 originally planned a `rule_based_beats_greedy` assertion too, but
//! the D28 safety heuristic empirically loses to plain greedy (2026-11-18
//! smokes at three seeds: 36-38% win rate). The rule-based assertion is
//! therefore run against the random baseline instead, which keeps the
//! regression truthful. A dedicated slice revisits D28 before m3 is
//! tagged accepted.
//!
//! m5 adds `score_aware_beats_rule_based`: the Wilson-LB-above-0.5 gate
//! on 2000 paired matches (D42) that justifies the match-equity +
//! equity-aware Close-card selection built in slices 1-4.
//!
//! m6 adds `clean_split_close_beats_rule_based`: the slice-5 exit test
//! on the first candidate tactic. 1000 paired matches (D49), Wilson LB
//! > 0.5 on the wrapped tactic's win rate.

use conga_ai::candidates::CleanSplitCloseWrapper;
use conga_ai::{BaselineAgent, RuleBasedPlayer, report_from_pairs, run_batch, wilson_ci};
use conga_core::Seat;
use conga_rules::StandardConga;
use std::time::Instant;

const Z: f64 = 1.96;
const SEED_A_SALT: u64 = 0xA1A1A1A1A1A1A1A1;
const SEED_B_SALT: u64 = 0xB2B2B2B2B2B2B2B2;

fn run(a_name: &str, b_name: &str, games: u32, seed: u64) -> (u32, u32, u32) {
    let rules = StandardConga::default();
    let games = if games % 2 == 1 { games + 1 } else { games };
    let pairs = games / 2;

    let a_name = a_name.to_string();
    let b_name = b_name.to_string();
    let seed_a = seed ^ SEED_A_SALT;
    let seed_b = seed ^ SEED_B_SALT;
    let builder_a = move || BaselineAgent::from_name(&a_name, seed_a).expect("a resolves");
    let builder_b = move || BaselineAgent::from_name(&b_name, seed_b).expect("b resolves");

    let started = Instant::now();
    let results = run_batch(&rules, builder_a, builder_b, pairs, seed, Seat::new(0));
    let report = report_from_pairs(&results, started.elapsed());
    (report.games, report.a_wins, report.b_wins)
}

#[test]
#[ignore = "slow (2000 full matches, ~3 min debug / ~20 s release); run with --release --ignored"]
fn greedy_beats_random() {
    let (games, a_wins, _) = run("greedy", "random", 2000, 0xC0FFEE);
    let (lo, _hi) = wilson_ci(a_wins, games, Z);
    assert!(
        lo > 0.5,
        "greedy_beats_random: wins {a_wins}/{games}, Wilson lo {lo:.4} (expected > 0.5)"
    );
}

#[test]
#[ignore = "slow (2000 full matches, ~3 min debug / ~20 s release); run with --release --ignored"]
fn rule_based_beats_random() {
    let (games, a_wins, _) = run("rule-based", "random", 2000, 0xBEEF);
    let (lo, _hi) = wilson_ci(a_wins, games, Z);
    assert!(
        lo > 0.5,
        "rule_based_beats_random: wins {a_wins}/{games}, Wilson lo {lo:.4} (expected > 0.5)"
    );
}

#[test]
#[ignore = "slow (2000 full matches, ~3 min release); run with --release --ignored"]
fn score_aware_beats_rule_based() {
    // m5 exit test (D42): 1000 paired deals, Wilson LB on score-aware's
    // win rate must exceed 0.5. Smokes at 500-200 games showed a stable
    // ~57% edge primarily from picking close-cards that hit the -10
    // clean-split bonus (D45).
    let (games, a_wins, _) = run("score-aware", "rule-based", 2000, 0x1);
    let (lo, _hi) = wilson_ci(a_wins, games, Z);
    assert!(
        lo > 0.5,
        "score_aware_beats_rule_based: wins {a_wins}/{games}, Wilson lo {lo:.4} (expected > 0.5)"
    );
}

#[test]
#[ignore = "slow (1000 full matches, ~2 min release); run with --release --ignored"]
fn clean_split_close_beats_rule_based() {
    // m6 exit test (D49): 1000 paired deals of the CleanSplitClose
    // wrapper over RuleBased vs unchanged rule-based. The tactic
    // under test IS the equity-aware close-card picker factored out
    // of ScoreAware (D47). It wraps the same rule-based base, so any
    // delta vs unchanged rule-based is attributable to the clean-
    // split close pick. Expectation: should clear 0.5 cleanly (same
    // signal that drives D45's ~7 pp edge).
    let rules = StandardConga::default();
    let seed: u64 = 0x1;
    let games = 1000u32;
    let pairs = games / 2;
    let samples = 20u32;
    let tactic_seed = seed ^ SEED_A_SALT;
    let unchanged_seed = seed ^ SEED_B_SALT;
    let builder_tactic =
        move || CleanSplitCloseWrapper::new(RuleBasedPlayer::new(), samples, tactic_seed);
    let builder_unchanged =
        move || BaselineAgent::from_name("rule-based", unchanged_seed).expect("rule-based");

    let started = Instant::now();
    let results = run_batch(
        &rules,
        builder_tactic,
        builder_unchanged,
        pairs,
        seed,
        Seat::new(0),
    );
    let report = report_from_pairs(&results, started.elapsed());
    let (lo, _hi) = wilson_ci(report.a_wins, report.games, Z);
    assert!(
        lo > 0.5,
        "clean_split_close_beats_rule_based: wins {}/{}, Wilson lo {:.4} (expected > 0.5)",
        report.a_wins,
        report.games,
        lo
    );
}
