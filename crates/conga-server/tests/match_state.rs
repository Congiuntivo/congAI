//! Unit tests for `MatchState`. Uses `StandardConga` as the rule set so
//! the `match_status` call goes through the real rule-16 logic.

use conga_core::{Bonus, HandResult, MatchStatus, Seat};
use conga_rules::StandardConga;
use conga_server::MatchState;

fn rules() -> StandardConga {
    StandardConga::default()
}

#[test]
fn new_initialises_scores_dealer_and_status() {
    let m = MatchState::new(2, Seat::new(1));
    assert_eq!(m.seats_count, 2);
    assert_eq!(m.scores, vec![0, 0]);
    assert_eq!(m.dealer, Seat::new(1));
    assert_eq!(m.hands_played, 0);
    assert_eq!(m.status, MatchStatus::InProgress);
    assert!(!m.is_finished());
}

#[test]
fn finish_hand_folds_points_bumps_count_rotates_dealer() {
    let r = rules();
    let mut m = MatchState::new(3, Seat::new(0));
    let hand = HandResult {
        points: vec![5, 12, -10],
        closer: Some(Seat::new(2)),
        bonus: Some(Bonus::MinusTen),
    };
    m.finish_hand(&r, &hand);
    assert_eq!(m.scores, vec![5, 12, -10]);
    assert_eq!(m.hands_played, 1);
    assert_eq!(m.dealer, Seat::new(1));
    assert_eq!(m.status, MatchStatus::InProgress);
}

#[test]
fn dealer_rotates_through_every_seat() {
    let r = rules();
    let mut m = MatchState::new(4, Seat::new(0));
    let zero = HandResult {
        points: vec![0, 0, 0, 0],
        closer: None,
        bonus: None,
    };
    for i in 0..4 {
        m.finish_hand(&r, &zero);
        assert_eq!(m.dealer, Seat::new(((i as u8) + 1) % 4));
    }
    assert_eq!(m.hands_played, 4);
}

#[test]
fn finish_hand_sets_status_to_winner_when_someone_passes_100() {
    let r = rules();
    let mut m = MatchState::new(2, Seat::new(0));
    let hand = HandResult {
        points: vec![0, 101],
        closer: Some(Seat::new(0)),
        bonus: None,
    };
    m.finish_hand(&r, &hand);
    assert_eq!(m.status, MatchStatus::Winner(Seat::new(0)));
    assert!(m.is_finished());
}

#[test]
fn rule_16_tie_break_picks_lowest_score_on_simultaneous_elimination() {
    let r = rules();
    let mut m = MatchState::new(3, Seat::new(0));
    // Three-way simultaneous elimination, seat 2 has the lowest total.
    let hand = HandResult {
        points: vec![110, 105, 102],
        closer: None,
        bonus: None,
    };
    m.finish_hand(&r, &hand);
    match m.status {
        MatchStatus::Winner(seat) => assert_eq!(seat, Seat::new(2)),
        other => panic!("expected Winner(seat 2), got {other:?}"),
    }
}

#[test]
#[should_panic(expected = "HandResult has wrong seat count")]
fn finish_hand_rejects_wrong_seat_count() {
    let r = rules();
    let mut m = MatchState::new(2, Seat::new(0));
    let hand = HandResult {
        points: vec![1, 2, 3],
        closer: None,
        bonus: None,
    };
    m.finish_hand(&r, &hand);
}

#[test]
#[should_panic(expected = "seats_count must be in 2..=4")]
fn new_rejects_one_seat() {
    let _ = MatchState::new(1, Seat::new(0));
}

#[test]
#[should_panic(expected = "out of range")]
fn new_rejects_dealer_outside_range() {
    let _ = MatchState::new(2, Seat::new(2));
}
