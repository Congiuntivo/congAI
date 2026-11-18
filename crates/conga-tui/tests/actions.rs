//! Resolver: KeyIntent + CursorPos + legal → Resolved.

use conga_core::{Action, Card, Hand, Observation, Phase, Rank, Seat, Suit};
use conga_tui::KeyIntent;
use conga_tui::actions::{Resolved, resolve};
use conga_tui::app::CursorPos;
use conga_tui::suits::SuitTheme;

fn c(suit: Suit, rank: u8) -> Card {
    Card::new(suit, Rank::new(rank).unwrap())
}

fn obs_with_hand(cards: &[Card]) -> Observation {
    Observation {
        own_seat: Seat::new(0),
        current_seat: Seat::new(0),
        phase: Phase::Discard,
        own_hand: Hand::from(cards),
        top_discard: Some(c(Suit::Spades, 5)),
        discard_history: vec![c(Suit::Spades, 5)],
        stock_size: 25,
        melds_on_table: Vec::new(),
        scores: vec![0, 0],
        hand_sizes: vec![cards.len() as u8, 7],
    }
}

fn is_accept(r: &Resolved, expected: &Action) -> bool {
    matches!(r, Resolved::Accept(a) if a == expected)
}

fn reject_msg(r: &Resolved) -> &str {
    match r {
        Resolved::Reject(m) => m.as_str(),
        _ => panic!("expected Reject, got something else"),
    }
}

#[test]
fn enter_on_stock_resolves_to_draw_stock_when_legal() {
    let obs = obs_with_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock, Action::DrawDiscard];
    let r = resolve(
        KeyIntent::Activate,
        CursorPos::Stock,
        &obs,
        &legal,
        SuitTheme::French,
    );
    assert!(is_accept(&r, &Action::DrawStock));
}

#[test]
fn enter_on_discard_resolves_to_draw_discard_when_legal() {
    let obs = obs_with_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock, Action::DrawDiscard];
    let r = resolve(
        KeyIntent::Activate,
        CursorPos::Discard,
        &obs,
        &legal,
        SuitTheme::French,
    );
    assert!(is_accept(&r, &Action::DrawDiscard));
}

#[test]
fn enter_on_hand_is_the_discard_of_that_card() {
    let hand = [c(Suit::Clubs, 1), c(Suit::Hearts, 2)];
    let obs = obs_with_hand(&hand);
    // Hand::iter yields ascending card indices; sorted order is
    // 1♣ (index 0) then 2♥ (index 11). CursorPos::Hand(1) means the
    // SECOND card in the sorted iter — 2♥.
    let sorted: Vec<Card> = obs.own_hand.iter().collect();
    assert_eq!(sorted.len(), 2);
    let second = sorted[1];
    let legal = vec![Action::Discard(second), Action::Discard(sorted[0])];
    let r = resolve(
        KeyIntent::Activate,
        CursorPos::Hand(1),
        &obs,
        &legal,
        SuitTheme::French,
    );
    assert!(is_accept(&r, &Action::Discard(second)));
}

#[test]
fn enter_on_stock_rejects_when_not_in_legal() {
    let obs = obs_with_hand(&[c(Suit::Clubs, 1)]);
    let legal: Vec<Action> = vec![Action::DrawDiscard];
    let r = resolve(
        KeyIntent::Activate,
        CursorPos::Stock,
        &obs,
        &legal,
        SuitTheme::French,
    );
    let msg = reject_msg(&r);
    assert!(msg.contains("stock"), "unexpected: {msg}");
}

#[test]
fn c_on_hand_resolves_to_close_when_legal() {
    let hand = [c(Suit::Clubs, 1), c(Suit::Hearts, 2)];
    let obs = obs_with_hand(&hand);
    let sorted: Vec<Card> = obs.own_hand.iter().collect();
    let legal = vec![Action::Close(sorted[0])];
    let r = resolve(
        KeyIntent::CloseSelected,
        CursorPos::Hand(0),
        &obs,
        &legal,
        SuitTheme::French,
    );
    assert!(is_accept(&r, &Action::Close(sorted[0])));
}

#[test]
fn p_resolves_to_pass_when_legal() {
    let obs = obs_with_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::Pass];
    let r = resolve(
        KeyIntent::Pass,
        CursorPos::Hand(0),
        &obs,
        &legal,
        SuitTheme::French,
    );
    assert!(is_accept(&r, &Action::Pass));
}

#[test]
fn move_intents_return_movement_variants() {
    let obs = obs_with_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock];
    let l = resolve(
        KeyIntent::MoveLeft,
        CursorPos::Discard,
        &obs,
        &legal,
        SuitTheme::French,
    );
    let r = resolve(
        KeyIntent::MoveRight,
        CursorPos::Stock,
        &obs,
        &legal,
        SuitTheme::French,
    );
    assert!(matches!(l, Resolved::MoveLeft));
    assert!(matches!(r, Resolved::MoveRight));
}

#[test]
fn quit_intent_returns_quit() {
    let obs = obs_with_hand(&[c(Suit::Clubs, 1)]);
    let legal: Vec<Action> = vec![];
    let q = resolve(
        KeyIntent::Quit,
        CursorPos::Stock,
        &obs,
        &legal,
        SuitTheme::French,
    );
    assert!(matches!(q, Resolved::Quit));
}

#[test]
fn ignored_intent_is_noop() {
    let obs = obs_with_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock];
    let n = resolve(
        KeyIntent::Ignored,
        CursorPos::Stock,
        &obs,
        &legal,
        SuitTheme::French,
    );
    assert!(matches!(n, Resolved::NoOp));
}
