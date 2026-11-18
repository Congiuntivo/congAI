//! Cursor movement + initial-slot heuristic.

use conga_core::{Action, Card, Hand, Meld, MeldKind, Observation, Phase, Rank, Seat, Suit};
use conga_tui::app::{AppState, CursorPos, Mode, initial_cursor, rebuild_hand_order};
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
    let obs = obs_with_hand(cards);
    s.update_hand_order(obs.own_hand);
    s.observation = Some(obs);
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

fn set_meld(cards: &[Card]) -> Meld {
    Meld {
        kind: MeldKind::Set,
        cards: Hand::from(cards),
    }
}

#[test]
fn enter_lay_meld_mode_filters_to_lay_meld_variants() {
    let m1 = set_meld(&[c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)]);
    let m2 = set_meld(&[c(Suit::Clubs, 3), c(Suit::Diamonds, 3), c(Suit::Hearts, 3)]);
    let hand = [c(Suit::Clubs, 7)];
    let mut s = state_with_hand(
        &hand,
        vec![
            Action::DrawStock,
            Action::LayMeld(m1.clone()),
            Action::LayMeld(m2.clone()),
            Action::Pass,
        ],
    );
    assert!(s.enter_lay_meld_mode());
    match &s.mode {
        Mode::LayMeldCycle { variants, index } => {
            assert_eq!(variants.len(), 2);
            assert_eq!(*index, 0);
            assert!(matches!(&variants[0], Action::LayMeld(m) if m == &m1));
            assert!(matches!(&variants[1], Action::LayMeld(m) if m == &m2));
        }
        other => panic!("expected LayMeldCycle, got {other:?}"),
    }
}

#[test]
fn enter_lay_meld_mode_fails_when_no_lay_meld_variants() {
    let hand = [c(Suit::Clubs, 7)];
    let mut s = state_with_hand(&hand, vec![Action::DrawStock]);
    assert!(!s.enter_lay_meld_mode());
    assert!(matches!(s.mode, Mode::Normal));
}

#[test]
fn cycle_next_wraps_around() {
    let m1 = set_meld(&[c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)]);
    let m2 = set_meld(&[c(Suit::Clubs, 3), c(Suit::Diamonds, 3), c(Suit::Hearts, 3)]);
    let hand = [c(Suit::Clubs, 7)];
    let mut s = state_with_hand(&hand, vec![Action::LayMeld(m1), Action::LayMeld(m2)]);
    s.enter_lay_meld_mode();
    s.cycle_next();
    if let Mode::LayMeldCycle { index, .. } = s.mode {
        assert_eq!(index, 1);
    } else {
        panic!("not in cycle mode");
    }
    s.cycle_next();
    if let Mode::LayMeldCycle { index, .. } = s.mode {
        assert_eq!(index, 0, "should wrap back to 0");
    } else {
        panic!("not in cycle mode");
    }
}

#[test]
fn cycle_prev_wraps_around() {
    let m1 = set_meld(&[c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)]);
    let m2 = set_meld(&[c(Suit::Clubs, 3), c(Suit::Diamonds, 3), c(Suit::Hearts, 3)]);
    let hand = [c(Suit::Clubs, 7)];
    let mut s = state_with_hand(&hand, vec![Action::LayMeld(m1), Action::LayMeld(m2)]);
    s.enter_lay_meld_mode();
    s.cycle_prev();
    if let Mode::LayMeldCycle { index, .. } = s.mode {
        assert_eq!(index, 1, "prev at 0 wraps to last");
    } else {
        panic!("not in cycle mode");
    }
}

#[test]
fn current_cycle_cards_lists_the_meld_cards() {
    let meld_cards = vec![c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)];
    let m = set_meld(&meld_cards);
    let hand = [c(Suit::Clubs, 7)];
    let mut s = state_with_hand(&hand, vec![Action::LayMeld(m)]);
    s.enter_lay_meld_mode();
    let cards = s.current_cycle_cards();
    assert_eq!(cards, meld_cards);
}

#[test]
fn enter_add_meld_mode_filters_to_add_variants() {
    let card = c(Suit::Hearts, 10);
    let hand = [card];
    let mut s = state_with_hand(
        &hand,
        vec![
            Action::DrawStock,
            Action::AddToMeld {
                card,
                meld_index: 0,
            },
            Action::AddToMeld {
                card,
                meld_index: 1,
            },
        ],
    );
    assert!(s.enter_add_meld_mode());
    if let Mode::AddToMeldCycle { variants, .. } = &s.mode {
        assert_eq!(variants.len(), 2);
    } else {
        panic!("expected AddToMeldCycle");
    }
    // current_cycle_cards for AddToMeld is just the single card.
    let cards = s.current_cycle_cards();
    assert_eq!(cards, vec![card]);
}

#[test]
fn exit_mode_returns_to_normal() {
    let m = set_meld(&[c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)]);
    let hand = [c(Suit::Clubs, 7)];
    let mut s = state_with_hand(&hand, vec![Action::LayMeld(m)]);
    s.enter_lay_meld_mode();
    assert!(s.is_in_cycle_mode());
    s.exit_mode();
    assert!(!s.is_in_cycle_mode());
    assert!(matches!(s.mode, Mode::Normal));
}

#[test]
fn rebuild_hand_order_preserves_prior_order_and_appends_new_cards() {
    let a = c(Suit::Clubs, 1);
    let b = c(Suit::Clubs, 2);
    let d = c(Suit::Clubs, 4);
    let e = c(Suit::Clubs, 5);
    let prior = vec![a, b, d];
    // New hand drops b and adds e.
    let new_hand = Hand::from(&[a, d, e][..]);
    let next = rebuild_hand_order(&prior, new_hand);
    assert_eq!(next, vec![a, d, e]);
}

#[test]
fn rebuild_hand_order_is_stable_when_hand_is_unchanged() {
    // Pick cards that span suits so the default Hand::iter order
    // differs from the prior order we feed in.
    let a = c(Suit::Clubs, 3);
    let b = c(Suit::Hearts, 5);
    let d = c(Suit::Spades, 2);
    let prior = vec![b, d, a];
    let new_hand = Hand::from(&[a, b, d][..]);
    let next = rebuild_hand_order(&prior, new_hand);
    assert_eq!(next, vec![b, d, a], "stable order expected");
}

#[test]
fn rebuild_hand_order_from_empty_prior_uses_hand_iter_order() {
    let a = c(Suit::Clubs, 3);
    let b = c(Suit::Hearts, 5);
    let new_hand = Hand::from(&[a, b][..]);
    let next = rebuild_hand_order(&[], new_hand);
    assert_eq!(next, new_hand.iter().collect::<Vec<_>>());
}

#[test]
fn swap_cursor_right_swaps_neighbors_and_cursor_follows() {
    let hand = [c(Suit::Clubs, 1), c(Suit::Clubs, 2), c(Suit::Clubs, 3)];
    let mut s = state_with_hand(&hand, vec![Action::DrawStock]);
    s.cursor = CursorPos::Hand(1);
    let before = s.hand_order.clone();
    s.swap_cursor_right();
    // hand_order[1] and hand_order[2] swapped; cursor moved to 2.
    assert_eq!(s.cursor, CursorPos::Hand(2));
    assert_eq!(s.hand_order[1], before[2]);
    assert_eq!(s.hand_order[2], before[1]);
}

#[test]
fn swap_cursor_left_swaps_neighbors_and_cursor_follows() {
    let hand = [c(Suit::Clubs, 1), c(Suit::Clubs, 2), c(Suit::Clubs, 3)];
    let mut s = state_with_hand(&hand, vec![Action::DrawStock]);
    s.cursor = CursorPos::Hand(2);
    let before = s.hand_order.clone();
    s.swap_cursor_left();
    assert_eq!(s.cursor, CursorPos::Hand(1));
    assert_eq!(s.hand_order[1], before[2]);
    assert_eq!(s.hand_order[2], before[1]);
}

#[test]
fn swap_cursor_right_at_last_card_is_a_noop() {
    let hand = [c(Suit::Clubs, 1), c(Suit::Clubs, 2)];
    let mut s = state_with_hand(&hand, vec![Action::DrawStock]);
    s.cursor = CursorPos::Hand(1);
    let before = s.hand_order.clone();
    s.swap_cursor_right();
    assert_eq!(s.cursor, CursorPos::Hand(1));
    assert_eq!(s.hand_order, before);
}

#[test]
fn swap_cursor_left_at_first_card_is_a_noop() {
    let hand = [c(Suit::Clubs, 1), c(Suit::Clubs, 2)];
    let mut s = state_with_hand(&hand, vec![Action::DrawStock]);
    s.cursor = CursorPos::Hand(0);
    let before = s.hand_order.clone();
    s.swap_cursor_left();
    assert_eq!(s.cursor, CursorPos::Hand(0));
    assert_eq!(s.hand_order, before);
}

#[test]
fn swap_cursor_on_non_hand_position_is_a_noop() {
    let hand = [c(Suit::Clubs, 1), c(Suit::Clubs, 2)];
    let mut s = state_with_hand(&hand, vec![Action::DrawStock]);
    s.cursor = CursorPos::Stock;
    let before = s.hand_order.clone();
    s.swap_cursor_right();
    s.swap_cursor_left();
    assert_eq!(s.cursor, CursorPos::Stock);
    assert_eq!(s.hand_order, before);
}

#[test]
fn hand_card_at_cursor_reads_from_hand_order() {
    let hand = [c(Suit::Clubs, 1), c(Suit::Hearts, 2)];
    let mut s = state_with_hand(&hand, vec![Action::DrawStock]);
    // Default order is Hand::iter order (ascending card index): 1♣ then 2♥.
    assert_eq!(s.hand_order, vec![c(Suit::Clubs, 1), c(Suit::Hearts, 2)]);
    s.cursor = CursorPos::Hand(0);
    assert_eq!(s.hand_card_at_cursor(), Some(c(Suit::Clubs, 1)));
    s.swap_cursor_right();
    // Cursor now at Hand(1); card under it is still 1♣ (the swapped one).
    assert_eq!(s.cursor, CursorPos::Hand(1));
    assert_eq!(s.hand_card_at_cursor(), Some(c(Suit::Clubs, 1)));
}
