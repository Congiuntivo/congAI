//! Diagnostic experiments on `SearchPlayer`.
//!
//! Each #[ignore]d test runs 20 paired deals (40 matches) of a search
//! variant versus greedy and prints the Wilson CI on the search's win
//! rate. Used to decide whether PIMC can be salvaged in m4 before
//! investing in slice 5 regressions.
//!
//! Run with `cargo test -p conga-ai --test search_experiments
//! --release -- --ignored --nocapture`.

use std::time::Instant;

use conga_ai::{
    BaselineAgent, EvalReport, GreedyPlayer, RolloutPolicy, SearchPlayer, report_from_pairs,
    run_batch,
};
use conga_core::Seat;
use conga_rules::StandardConga;

const SEARCH_SEED: u64 = 0xA1A1_A1A1_A1A1_A1A1;
const MATCH_SEED: u64 = 0x1;

fn print_report(label: &str, report: &EvalReport) {
    let (lo, hi) = report.a_win_rate_ci();
    eprintln!(
        "{label}: {}/{} wins ({:.1}%, 95% CI [{:.1}%, {:.1}%])  mean score {:.1} vs {:.1}  hands/match {:.2}  decisions/s {:.0}  duration {}.{:03}s",
        report.a_wins,
        report.games,
        report.a_win_rate() * 100.0,
        lo * 100.0,
        hi * 100.0,
        report.mean_a_score,
        report.mean_b_score,
        report.hands_per_match,
        report.decisions_per_second,
        report.duration_ms / 1000,
        report.duration_ms % 1000,
    );
}

#[test]
#[ignore = "diagnostic: search + random rollouts vs greedy, 20 paired deals"]
fn search_random_rollouts_vs_greedy() {
    let rules = StandardConga::default();
    let start = Instant::now();
    let pairs = run_batch(
        &rules,
        || SearchPlayer::new(20, SEARCH_SEED).with_rollout_policy(RolloutPolicy::Random),
        || BaselineAgent::Greedy(GreedyPlayer::new()),
        20,
        MATCH_SEED,
        Seat::new(1),
    );
    let elapsed = start.elapsed();
    let report = report_from_pairs(&pairs, elapsed);
    print_report("search(random-rollouts) vs greedy", &report);
}

#[test]
#[ignore = "diagnostic: search + opp-weight 0.3 vs greedy, 20 paired deals"]
fn search_opp_weight_vs_greedy() {
    let rules = StandardConga::default();
    let start = Instant::now();
    let pairs = run_batch(
        &rules,
        || SearchPlayer::new(20, SEARCH_SEED).with_opponent_weight(0.3),
        || BaselineAgent::Greedy(GreedyPlayer::new()),
        20,
        MATCH_SEED,
        Seat::new(1),
    );
    let elapsed = start.elapsed();
    let report = report_from_pairs(&pairs, elapsed);
    print_report("search(opp-weight 0.3) vs greedy", &report);
}
