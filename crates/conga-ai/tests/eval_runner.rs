//! Headless match runner: plays `RandomPlayer` vs `RandomPlayer` and
//! `GreedyPlayer` vs `RandomPlayer` through the engine directly.

use conga_ai::{GreedyPlayer, RandomPlayer, run_batch, run_match, run_paired};
use conga_core::{MatchStatus, Seat};
use conga_rules::StandardConga;

#[test]
fn run_match_produces_a_winner_from_random_vs_random() {
    let rules = StandardConga::default();
    let mut a = RandomPlayer::from_seed(1);
    let mut b = RandomPlayer::from_seed(2);
    let o = run_match(&rules, &mut a, &mut b, Seat::new(1), 0xC0FFEE);
    assert!(
        o.winner_seat.is_some(),
        "random match must end with a winner"
    );
    assert_eq!(o.final_scores.len(), 2);
    assert!(o.hands_played >= 1);
    assert!(o.decisions > 0);
}

#[test]
fn run_batch_of_random_vs_random_all_finish() {
    let rules = StandardConga::default();
    let results = run_batch(
        &rules,
        || RandomPlayer::from_seed(1),
        || RandomPlayer::from_seed(2),
        5,
        0xC0FFEE,
        Seat::new(1),
    );
    assert_eq!(results.len(), 5);
    for pair in &results {
        assert!(pair.a_on_seat_0.winner_seat.is_some());
        assert!(pair.a_on_seat_1.winner_seat.is_some());
    }
}

#[test]
fn run_batch_is_reproducible_from_the_seed() {
    let rules = StandardConga::default();
    let a_seed = 1;
    let b_seed = 2;
    let r1 = run_batch(
        &rules,
        || RandomPlayer::from_seed(a_seed),
        || RandomPlayer::from_seed(b_seed),
        3,
        0xABC,
        Seat::new(1),
    );
    let r2 = run_batch(
        &rules,
        || RandomPlayer::from_seed(a_seed),
        || RandomPlayer::from_seed(b_seed),
        3,
        0xABC,
        Seat::new(1),
    );
    assert_eq!(r1, r2);
}

#[test]
fn paired_matches_share_a_seed_but_produce_distinct_outcomes_with_distinct_agents() {
    // With two distinguishable deterministic agents (greedy vs seeded
    // random), swapping seats on the same chance stream should almost
    // never produce identical outcomes.
    let rules = StandardConga::default();
    let pair = run_paired(
        &rules,
        GreedyPlayer::new,
        || RandomPlayer::from_seed(7),
        Seat::new(1),
        0xDEAD,
    );
    // Both matches finished.
    assert!(pair.a_on_seat_0.winner_seat.is_some());
    assert!(pair.a_on_seat_1.winner_seat.is_some());
    // And they used the same chance seed but different seat-0 agent,
    // so the sequences of decisions diverge: the two outcomes will
    // differ in at least one of (winner, scores, hands_played).
    assert_ne!(
        (
            &pair.a_on_seat_0.winner_seat,
            &pair.a_on_seat_0.final_scores,
            pair.a_on_seat_0.hands_played
        ),
        (
            &pair.a_on_seat_1.winner_seat,
            &pair.a_on_seat_1.final_scores,
            pair.a_on_seat_1.hands_played
        ),
        "expected the paired matches to diverge"
    );
}

#[test]
fn a_wins_and_b_wins_add_up_to_pair_size() {
    let rules = StandardConga::default();
    let results = run_batch(
        &rules,
        GreedyPlayer::new,
        || RandomPlayer::from_seed(11),
        10,
        0xBEEF,
        Seat::new(1),
    );
    for pair in &results {
        assert_eq!(pair.a_wins() + pair.b_wins(), 2);
    }
    // Over 10 paired matches, greedy should win the clear majority
    // against random — this is a soft signal before the real
    // regression test in slice 6.
    let total_a = results.iter().map(|p| p.a_wins()).sum::<u32>();
    let total_b = results.iter().map(|p| p.b_wins()).sum::<u32>();
    assert!(
        total_a > total_b,
        "greedy should beat random over 10 paired matches (got {total_a} vs {total_b})"
    );
}

#[test]
fn match_status_returns_winner_not_in_progress_at_end_of_match() {
    let rules = StandardConga::default();
    let mut a = RandomPlayer::from_seed(1);
    let mut b = RandomPlayer::from_seed(2);
    let o = run_match(&rules, &mut a, &mut b, Seat::new(1), 0xC0FFEE);
    match o.winner_seat {
        Some(_) => {}
        None => panic!("match ended with MatchStatus::InProgress"),
    }
    // Sanity: score layout matches seat count.
    assert_eq!(o.final_scores.len(), 2);
    // Sanity: rules confirm the status.
    use conga_core::RuleSet;
    assert!(matches!(
        rules.match_status(&o.final_scores),
        MatchStatus::Winner(_)
    ));
}
