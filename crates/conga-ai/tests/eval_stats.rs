//! `wilson_ci` + `report_from_pairs` + `BaselineAgent`.

use std::time::Duration;

use conga_ai::{BaselineAgent, MatchOutcome, PairedResult, Player, report_from_pairs, wilson_ci};
use conga_core::{Action, Hand, Observation, Phase, Seat};

#[test]
fn wilson_ci_at_50_of_100_is_approximately_centered() {
    let (lo, hi) = wilson_ci(50, 100, 1.96);
    assert!(lo > 0.39 && lo < 0.41, "lo = {lo}");
    assert!(hi > 0.59 && hi < 0.61, "hi = {hi}");
}

#[test]
fn wilson_ci_at_full_sweep_clamps_the_upper_bound() {
    let (lo, hi) = wilson_ci(100, 100, 1.96);
    // With p = 1.0 the Wilson upper bound is 1.0 mathematically; the
    // float arithmetic stops a ULP short. Allow either.
    assert!(lo > 0.96 && lo < 1.0, "lo = {lo}");
    assert!(hi > 0.999 && hi <= 1.0, "hi = {hi}");
}

#[test]
fn wilson_ci_at_zero_wins_clamps_the_lower_bound() {
    let (lo, hi) = wilson_ci(0, 100, 1.96);
    assert_eq!(lo, 0.0);
    assert!(hi > 0.0 && hi < 0.05, "hi = {hi}");
}

#[test]
fn wilson_ci_on_zero_samples_is_vacuous() {
    assert_eq!(wilson_ci(0, 0, 1.96), (0.0, 1.0));
}

#[test]
fn baseline_agent_from_name_round_trips() {
    for name in ["random", "greedy", "rule-based"] {
        assert!(
            BaselineAgent::from_name(name, 42).is_some(),
            "expected {name} to resolve"
        );
    }
    assert!(BaselineAgent::from_name("mcts", 42).is_none());
    assert!(BaselineAgent::from_name("", 42).is_none());
}

#[test]
fn baseline_agent_forwards_to_the_inner_player() {
    // Greedy is deterministic: given the same hand + legal set, both a
    // bare GreedyPlayer and a BaselineAgent::Greedy must pick the same
    // action.
    use conga_ai::GreedyPlayer;
    use conga_core::{Card, Rank, Suit};

    let hand = [
        Card::new(Suit::Clubs, Rank::new(7).unwrap()),
        Card::new(Suit::Diamonds, Rank::new(7).unwrap()),
        Card::new(Suit::Hearts, Rank::new(7).unwrap()),
        Card::new(Suit::Spades, Rank::new(2).unwrap()),
        Card::new(Suit::Spades, Rank::new(10).unwrap()),
    ];
    let obs = Observation {
        own_seat: Seat::new(0),
        current_seat: Seat::new(0),
        phase: Phase::Discard,
        own_hand: Hand::from(hand.as_slice()),
        top_discard: None,
        discard_history: Vec::new(),
        stock_size: 20,
        melds_on_table: Vec::new(),
        scores: vec![0, 0],
        hand_sizes: vec![hand.len() as u8, 7],
    };
    let legal: Vec<Action> = hand.iter().map(|c| Action::Discard(*c)).collect();

    let mut bare = GreedyPlayer::new();
    let mut wrapped = BaselineAgent::from_name("greedy", 0).unwrap();
    assert_eq!(bare.act(&obs, &legal), wrapped.act(&obs, &legal));
}

fn outcome(winner: Option<Seat>, scores: [i32; 2], hands: u32, decisions: u64) -> MatchOutcome {
    MatchOutcome {
        winner_seat: winner,
        final_scores: scores.to_vec(),
        hands_played: hands,
        decisions,
    }
}

#[test]
fn report_counts_wins_losses_and_averages_scores() {
    // Pair 1: A (seat 0) wins the first match 101-50, loses the second
    // (as seat 1) 80-101. So across the pair: 1 win each, A score sum
    // = 101 + 80 = 181, B = 50 + 101 = 151.
    let pair = PairedResult {
        a_on_seat_0: outcome(Some(Seat::new(0)), [101, 50], 5, 40),
        a_on_seat_1: outcome(Some(Seat::new(0)), [101, 80], 6, 48),
    };
    let r = report_from_pairs(&[pair], Duration::from_millis(100));
    assert_eq!(r.games, 2);
    assert_eq!(r.a_wins, 1);
    assert_eq!(r.b_wins, 1);
    assert_eq!(r.draws, 0);
    assert!((r.mean_a_score - (101.0 + 80.0) / 2.0).abs() < 1e-9);
    assert!((r.mean_b_score - (50.0 + 101.0) / 2.0).abs() < 1e-9);
    assert!((r.hands_per_match - (5.0 + 6.0) / 2.0).abs() < 1e-9);
    assert!(r.decisions_per_second > 0.0);
    assert_eq!(r.duration_ms, 100);
}

#[test]
fn report_counts_a_wins_on_both_matches_when_a_dominates_a_pair() {
    // A wins match 1 as seat 0 AND match 2 as seat 1.
    let pair = PairedResult {
        a_on_seat_0: outcome(Some(Seat::new(0)), [101, 40], 5, 40),
        a_on_seat_1: outcome(Some(Seat::new(1)), [60, 101], 5, 40),
    };
    let r = report_from_pairs(&[pair], Duration::from_millis(10));
    assert_eq!(r.a_wins, 2);
    assert_eq!(r.b_wins, 0);
}

#[test]
fn report_treats_none_winners_as_draws() {
    let pair = PairedResult {
        a_on_seat_0: outcome(None, [50, 50], 3, 20),
        a_on_seat_1: outcome(None, [40, 40], 3, 20),
    };
    let r = report_from_pairs(&[pair], Duration::from_millis(1));
    assert_eq!(r.a_wins, 0);
    assert_eq!(r.b_wins, 0);
    assert_eq!(r.draws, 2);
}

#[test]
fn a_win_rate_ci_is_wilson_on_the_win_count() {
    let pair = PairedResult {
        a_on_seat_0: outcome(Some(Seat::new(0)), [101, 50], 5, 40),
        a_on_seat_1: outcome(Some(Seat::new(1)), [60, 101], 6, 48),
    };
    let r = report_from_pairs(&[pair], Duration::from_millis(10));
    // 2 wins out of 2 games: Wilson upper bound is 1.0, lower is the
    // computed value from wilson_ci.
    let (lo, hi) = r.a_win_rate_ci();
    let expected = wilson_ci(r.a_wins, r.games, 1.96);
    assert_eq!((lo, hi), expected);
}
