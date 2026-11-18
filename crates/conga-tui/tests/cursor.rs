//! Cursor movement + initial-slot heuristic.

use conga_core::{Action, Card, Hand, Observation, Phase, Rank, Seat, Suit};
use conga_tui::app::{AppState, CursorPos, initial_cursor};
use conga_tui::suits::SuitTheme;

fn c(suit: Suit, rank: u8) -> Card {
    Card::new(suit, Rank::new(rank).unwrap())
}

fn obs_with_hand(cards: &[Card]) -> Observation {
    Observation {
        own_seat: Seat::new(0),
        current_seat: Seat::new(0),
        phase: Phase::Draw,
        own_hand: Hand::from(cards),
        top_discard: Some(c(Suit::Spades, 5)),
        discard_history: vec![c(Suit::Spades, 5)],
        stock_size: 25,
        melds_on_table: Vec::new(),
        scores: vec![0, 0],
        hand_sizes: vec![cards.len() as u8, 7],
    }
}

fn state_with_hand(cards: &[Card], legal: Vec<Action>) -> AppState {
    let mut s = AppState::new(0, 2, SuitTheme::French);
    s.observation = Some(obs_with_hand(cards));
    s.legal = legal;
    s.reset_cursor();
    s
}

#[test]
fn initial_cursor_prefers_stock_when_draw_stock_is_legal() {
    let legal = vec![Action::DrawStock, Action::DrawDiscard];
    assert_eq!(initial_cursor(&legal, 7), CursorPos::Stock);
}

#[test]
fn initial_cursor_picks_discard_when_only_discard_draw_is_legal() {
    let legal = vec![Action::DrawDiscard];
    assert_eq!(initial_cursor(&legal, 7), CursorPos::Discard);
}

#[test]
fn initial_cursor_picks_first_hand_slot_when_no_draws_are_legal() {
    // Discard phase: only Discard(card) actions.
    let legal = vec![Action::Discard(c(Suit::Clubs, 3))];
    assert_eq!(initial_cursor(&legal, 7), CursorPos::Hand(0));
}

#[test]
fn initial_cursor_falls_back_to_stock_on_empty_hand_with_no_draws() {
    // Degenerate case — shouldn't happen in a real game but the
    // function must stay total.
    let legal: Vec<Action> = vec![];
    assert_eq!(initial_cursor(&legal, 0), CursorPos::Stock);
}

#[test]
fn move_left_from_stock_stays_at_stock() {
    let mut s = state_with_hand(
        &[c(Suit::Clubs, 1), c(Suit::Hearts, 2)],
        vec![Action::DrawStock],
    );
    assert_eq!(s.cursor, CursorPos::Stock);
    s.move_cursor_left();
    assert_eq!(s.cursor, CursorPos::Stock);
}

#[test]
fn move_right_walks_stock_discard_hand() {
    let hand = [c(Suit::Clubs, 1), c(Suit::Hearts, 2), c(Suit::Spades, 3)];
    let mut s = state_with_hand(&hand, vec![Action::DrawStock]);
    assert_eq!(s.cursor, CursorPos::Stock);
    s.move_cursor_right();
    assert_eq!(s.cursor, CursorPos::Discard);
    s.move_cursor_right();
    assert_eq!(s.cursor, CursorPos::Hand(0));
    s.move_cursor_right();
    assert_eq!(s.cursor, CursorPos::Hand(1));
    s.move_cursor_right();
    assert_eq!(s.cursor, CursorPos::Hand(2));
    // Past the last card: no movement.
    s.move_cursor_right();
    assert_eq!(s.cursor, CursorPos::Hand(2));
}

#[test]
fn move_left_from_hand_0_lands_on_discard() {
    let hand = [c(Suit::Clubs, 1)];
    let mut s = state_with_hand(&hand, vec![Action::DrawStock]);
    s.cursor = CursorPos::Hand(0);
    s.move_cursor_left();
    assert_eq!(s.cursor, CursorPos::Discard);
    s.move_cursor_left();
    assert_eq!(s.cursor, CursorPos::Stock);
}

#[test]
fn move_right_from_discard_stays_on_discard_if_hand_is_empty() {
    let mut s = state_with_hand(&[], vec![Action::DrawDiscard]);
    s.cursor = CursorPos::Discard;
    s.move_cursor_right();
    assert_eq!(s.cursor, CursorPos::Discard);
}
