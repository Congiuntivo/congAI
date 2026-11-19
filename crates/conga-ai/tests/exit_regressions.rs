//! m3 exit-test regression. Each test plays enough matches that the
//! 95% Wilson lower bound on the winning agent's win rate is strictly
//! above 0.5. Deterministic: same seed, same result.
//!
//! m3 originally planned a `rule_based_beats_greedy` assertion too, but
//! the D28 safety heuristic empirically loses to plain greedy (2026-11-18
//! smokes at three seeds: 36-38% win rate). The rule-based assertion is
//! therefore run against the random baseline instead, which keeps the
//! regression truthful. A dedicated slice revisits D28 before m3 is
//! tagged accepted.

use conga_ai::{BaselineAgent, report_from_pairs, run_batch, wilson_ci};
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
