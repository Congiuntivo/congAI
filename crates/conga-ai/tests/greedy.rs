//! `GreedyPlayer` behaviour on canned positions. All deterministic.

use conga_ai::{GreedyPlayer, Player};
use conga_core::{Action, Card, Hand, Meld, MeldKind, Observation, Phase, Rank, Seat, Suit};

fn c(suit: Suit, rank: u8) -> Card {
    Card::new(suit, Rank::new(rank).unwrap())
}

fn obs(hand: &[Card], phase: Phase, top: Option<Card>) -> Observation {
    Observation {
        own_seat: Seat::new(0),
        current_seat: Seat::new(0),
        phase,
        own_hand: Hand::from(hand),
        top_discard: top,
        discard_history: top.into_iter().collect(),
        stock_size: 25,
        melds_on_table: Vec::new(),
        scores: vec![0, 0],
        hand_sizes: vec![hand.len() as u8, 7],
    }
}

fn set_meld(cards: &[Card]) -> Meld {
    Meld {
        kind: MeldKind::Set,
        cards: Hand::from(cards),
    }
}

#[test]
fn close_takes_priority_when_legal() {
    let mut player = GreedyPlayer::new();
    let hand = [c(Suit::Clubs, 2)];
    let o = obs(&hand, Phase::Discard, None);
    let legal = vec![
        Action::DrawStock,
        Action::Discard(c(Suit::Clubs, 2)),
        Action::Close(c(Suit::Clubs, 2)),
        Action::Pass,
    ];
    let a = player.act(&o, &legal);
    assert!(matches!(a, Action::Close(_)), "got {a:?}");
}

#[test]
fn lay_meld_is_preferred_over_discard() {
    let mut player = GreedyPlayer::new();
    let hand = [
        c(Suit::Clubs, 7),
        c(Suit::Diamonds, 7),
        c(Suit::Hearts, 7),
        c(Suit::Spades, 2),
    ];
    let o = obs(&hand, Phase::Discard, None);
    let meld = set_meld(&[c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)]);
    let legal = vec![
        Action::Discard(c(Suit::Spades, 2)),
        Action::LayMeld(meld.clone()),
    ];
    let a = player.act(&o, &legal);
    assert_eq!(a, Action::LayMeld(meld));
}

#[test]
fn add_to_meld_is_preferred_over_discard() {
    let mut player = GreedyPlayer::new();
    let hand = [c(Suit::Clubs, 2), c(Suit::Spades, 9)];
    let o = obs(&hand, Phase::Discard, None);
    let add = Action::AddToMeld {
        card: c(Suit::Spades, 9),
        meld_index: 0,
    };
    let legal = vec![
        Action::Discard(c(Suit::Clubs, 2)),
        Action::Discard(c(Suit::Spades, 9)),
        add.clone(),
    ];
    let a = player.act(&o, &legal);
    assert_eq!(a, add);
}

#[test]
fn draw_discard_when_the_top_card_fits_a_meld() {
    // Hand: 7♣ 7♦ plus three low singletons. Top of discard is 7♥: it
    // completes a meld, so greedy should prefer the discard.
    let mut player = GreedyPlayer::new();
    let hand = [
        c(Suit::Clubs, 7),
        c(Suit::Diamonds, 7),
        c(Suit::Spades, 2),
        c(Suit::Spades, 3),
        c(Suit::Spades, 4),
        c(Suit::Hearts, 1),
        c(Suit::Hearts, 2),
    ];
    let top = c(Suit::Hearts, 7);
    let o = obs(&hand, Phase::Draw, Some(top));
    let legal = vec![Action::DrawStock, Action::DrawDiscard];
    let a = player.act(&o, &legal);
    assert_eq!(a, Action::DrawDiscard);
}

#[test]
fn draw_stock_when_the_top_card_does_not_fit() {
    // Hand has no near-meld for a 10♠; top is 10♠. The 10 does not
    // join anything; greedy should go to the stock.
    let mut player = GreedyPlayer::new();
    let hand = [
        c(Suit::Clubs, 1),
        c(Suit::Clubs, 2),
        c(Suit::Clubs, 3),
        c(Suit::Diamonds, 5),
        c(Suit::Hearts, 6),
        c(Suit::Hearts, 8),
        c(Suit::Spades, 4),
    ];
    let top = c(Suit::Spades, 10);
    let o = obs(&hand, Phase::Draw, Some(top));
    let legal = vec![Action::DrawStock, Action::DrawDiscard];
    let a = player.act(&o, &legal);
    assert_eq!(a, Action::DrawStock);
}

#[test]
fn discard_picks_the_highest_rank_unmatched_card() {
    // Hand: two melds of three (7s and 1s) plus two unmatched
    // singletons 10♠ and 2♠. Greedy discards the 10♠.
    let mut player = GreedyPlayer::new();
    let hand = [
        c(Suit::Clubs, 7),
        c(Suit::Diamonds, 7),
        c(Suit::Hearts, 7),
        c(Suit::Clubs, 1),
        c(Suit::Diamonds, 1),
        c(Suit::Hearts, 1),
        c(Suit::Spades, 2),
        c(Suit::Spades, 10),
    ];
    let o = obs(&hand, Phase::Discard, None);
    let legal: Vec<Action> = hand.iter().map(|card| Action::Discard(*card)).collect();
    let a = player.act(&o, &legal);
    assert_eq!(a, Action::Discard(c(Suit::Spades, 10)));
}

#[test]
fn pass_is_the_only_fallback_when_nothing_else_is_legal() {
    let mut player = GreedyPlayer::new();
    let hand = [c(Suit::Clubs, 1)];
    let o = obs(&hand, Phase::Discard, None);
    let legal = vec![Action::Pass];
    let a = player.act(&o, &legal);
    assert_eq!(a, Action::Pass);
}

#[test]
fn deterministic_same_observation_yields_the_same_action() {
    let mut p1 = GreedyPlayer::new();
    let mut p2 = GreedyPlayer::new();
    let hand = [
        c(Suit::Clubs, 7),
        c(Suit::Diamonds, 7),
        c(Suit::Spades, 2),
        c(Suit::Spades, 3),
        c(Suit::Spades, 4),
        c(Suit::Hearts, 1),
        c(Suit::Hearts, 2),
    ];
    let o = obs(&hand, Phase::Draw, Some(c(Suit::Hearts, 7)));
    let legal = vec![Action::DrawStock, Action::DrawDiscard];
    for _ in 0..50 {
        assert_eq!(p1.act(&o, &legal), p2.act(&o, &legal));
    }
}
