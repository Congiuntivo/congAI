//! `SearchPlayer` smoke: one match vs `GreedyPlayer`, logs rollouts/sec.
//!
//! Marked `#[ignore]`; run with `--release --ignored`.

use std::time::Instant;

use conga_ai::{GreedyPlayer, SearchPlayer, run_match};
use conga_core::Seat;
use conga_rules::StandardConga;

#[test]
#[ignore = "slow (one full match with K=20 search; ~1s release); run with --release --ignored"]
fn search_vs_greedy_one_match_logs_rollouts_per_sec() {
    let rules = StandardConga::default();
    let mut search = SearchPlayer::new(20, 42);
    let mut greedy = GreedyPlayer::new();

    let start = Instant::now();
    let outcome = run_match(&rules, &mut search, &mut greedy, Seat::new(1), 1);
    let elapsed = start.elapsed();

    let rollouts = search.rollouts();
    let stalemate = search.stalemate_rollouts();
    let own_close = search.own_close_rollouts();
    let opp_close = search.opp_close_rollouts();
    let secs = elapsed.as_secs_f64().max(1e-9);
    let pct = |n: u64| {
        if rollouts == 0 {
            0.0
        } else {
            100.0 * n as f64 / rollouts as f64
        }
    };
    eprintln!(
        "search vs greedy: hands={} decisions={} rollouts={} elapsed={:.3}s rollouts/sec={:.0}",
        outcome.hands_played,
        outcome.decisions,
        rollouts,
        secs,
        rollouts as f64 / secs,
    );
    eprintln!(
        "  rollout terminals: stalemate={} ({:.1}%)  own_close={} ({:.1}%)  opp_close={} ({:.1}%)",
        stalemate,
        pct(stalemate),
        own_close,
        pct(own_close),
        opp_close,
        pct(opp_close),
    );
    assert!(
        rollouts > 0,
        "expected non-zero rollouts (search was invoked)"
    );
}
