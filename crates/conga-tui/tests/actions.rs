//! Resolver: KeyIntent + CursorPos + hand_order + legal → Resolved.

use conga_core::{Action, Card, Hand, Rank, Suit};
use conga_tui::KeyIntent;
use conga_tui::actions::{CycleResolved, Resolved, resolve, resolve_cycle};
use conga_tui::app::CursorPos;
use conga_tui::suits::SuitTheme;

fn c(suit: Suit, rank: u8) -> Card {
    Card::new(suit, Rank::new(rank).unwrap())
}

/// Sorted display order for a hand, matching the default `hand_order`
/// populated by `update_hand_order` on the first `Observe`.
fn sorted_hand(cards: &[Card]) -> Vec<Card> {
    Hand::from(cards).iter().collect()
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
    let hand = sorted_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock, Action::DrawDiscard];
    let r = resolve(
        KeyIntent::Activate,
        CursorPos::Stock,
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(is_accept(&r, &Action::DrawStock));
}

#[test]
fn enter_on_discard_resolves_to_draw_discard_when_legal() {
    let hand = sorted_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock, Action::DrawDiscard];
    let r = resolve(
        KeyIntent::Activate,
        CursorPos::Discard,
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(is_accept(&r, &Action::DrawDiscard));
}

#[test]
fn enter_on_hand_is_the_discard_of_the_card_at_that_display_index() {
    // Pass a reordered hand: 2♥ first, 1♣ second. CursorPos::Hand(1)
    // should resolve to Discard(1♣), not Discard of the sorted-order
    // second card.
    let hand = vec![c(Suit::Hearts, 2), c(Suit::Clubs, 1)];
    let legal = vec![Action::Discard(hand[1]), Action::Discard(hand[0])];
    let r = resolve(
        KeyIntent::Activate,
        CursorPos::Hand(1),
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(is_accept(&r, &Action::Discard(hand[1])));
}

#[test]
fn enter_on_stock_rejects_when_not_in_legal() {
    let hand = sorted_hand(&[c(Suit::Clubs, 1)]);
    let legal: Vec<Action> = vec![Action::DrawDiscard];
    let r = resolve(
        KeyIntent::Activate,
        CursorPos::Stock,
        &hand,
        &legal,
        SuitTheme::French,
    );
    let msg = reject_msg(&r);
    assert!(msg.contains("stock"), "unexpected: {msg}");
}

#[test]
fn c_on_hand_resolves_to_close_when_legal() {
    let hand = sorted_hand(&[c(Suit::Clubs, 1), c(Suit::Hearts, 2)]);
    let legal = vec![Action::Close(hand[0])];
    let r = resolve(
        KeyIntent::CloseSelected,
        CursorPos::Hand(0),
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(is_accept(&r, &Action::Close(hand[0])));
}

#[test]
fn p_resolves_to_pass_when_legal() {
    let hand = sorted_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::Pass];
    let r = resolve(
        KeyIntent::Pass,
        CursorPos::Hand(0),
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(is_accept(&r, &Action::Pass));
}

#[test]
fn move_intents_return_movement_variants() {
    let hand = sorted_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock];
    let l = resolve(
        KeyIntent::MoveLeft,
        CursorPos::Discard,
        &hand,
        &legal,
        SuitTheme::French,
    );
    let r = resolve(
        KeyIntent::MoveRight,
        CursorPos::Stock,
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(matches!(l, Resolved::MoveLeft));
    assert!(matches!(r, Resolved::MoveRight));
}

#[test]
fn shift_arrow_intents_return_swap_variants() {
    let hand = sorted_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock];
    let l = resolve(
        KeyIntent::SwapLeft,
        CursorPos::Hand(0),
        &hand,
        &legal,
        SuitTheme::French,
    );
    let r = resolve(
        KeyIntent::SwapRight,
        CursorPos::Hand(0),
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(matches!(l, Resolved::SwapLeft));
    assert!(matches!(r, Resolved::SwapRight));
}

#[test]
fn quit_intent_returns_quit() {
    let hand: Vec<Card> = Vec::new();
    let legal: Vec<Action> = vec![];
    let q = resolve(
        KeyIntent::Quit,
        CursorPos::Stock,
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(matches!(q, Resolved::Quit));
}

#[test]
fn ignored_intent_is_noop() {
    let hand = sorted_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock];
    let n = resolve(
        KeyIntent::Ignored,
        CursorPos::Stock,
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(matches!(n, Resolved::NoOp));
}

#[test]
fn m_resolves_to_enter_lay_meld() {
    let hand = sorted_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock];
    let r = resolve(
        KeyIntent::MeldMode,
        CursorPos::Stock,
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(matches!(r, Resolved::EnterLayMeld));
}

#[test]
fn a_resolves_to_enter_add_meld() {
    let hand = sorted_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock];
    let r = resolve(
        KeyIntent::AddMode,
        CursorPos::Stock,
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(matches!(r, Resolved::EnterAddMeld));
}

#[test]
fn cancel_intent_resolves_to_cancel() {
    let hand = sorted_hand(&[c(Suit::Clubs, 1)]);
    let legal = vec![Action::DrawStock];
    let r = resolve(
        KeyIntent::Cancel,
        CursorPos::Stock,
        &hand,
        &legal,
        SuitTheme::French,
    );
    assert!(matches!(r, Resolved::Cancel));
}

#[test]
fn cycle_resolver_maps_the_cycle_keys() {
    assert_eq!(resolve_cycle(KeyIntent::MoveLeft), CycleResolved::Prev);
    assert_eq!(resolve_cycle(KeyIntent::MoveRight), CycleResolved::Next);
    assert_eq!(resolve_cycle(KeyIntent::Activate), CycleResolved::Commit);
    assert_eq!(resolve_cycle(KeyIntent::Cancel), CycleResolved::Cancel);
    assert_eq!(resolve_cycle(KeyIntent::Quit), CycleResolved::Quit);
    assert_eq!(resolve_cycle(KeyIntent::MeldMode), CycleResolved::Ignore);
    assert_eq!(resolve_cycle(KeyIntent::Pass), CycleResolved::Ignore);
    assert_eq!(resolve_cycle(KeyIntent::SwapLeft), CycleResolved::Ignore);
    assert_eq!(resolve_cycle(KeyIntent::SwapRight), CycleResolved::Ignore);
}
