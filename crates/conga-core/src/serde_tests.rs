//! Serde round-trip tests for the protocol-visible types.
//!
//! These live in a dedicated module so the round-trips are easy to find and
//! so the file can be gated on the `serde` feature in one place. The feature
//! is on by default (see `Cargo.toml`), so `cargo test --workspace` exercises
//! it without extra flags.

use std::borrow::Cow;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{
    Action, Bonus, Card, DrawSource, Event, GameLog, Hand, HandResult, MatchStatus, Meld, MeldKind,
    Observation, Phase, Rank, RuleSetId, Seat, Suit,
};

fn c(suit: Suit, rank: u8) -> Card {
    Card::new(suit, Rank::new(rank).unwrap())
}

fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: T) {
    let json = serde_json::to_string(&value).expect("serialize");
    let back: T = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(value, back, "round-trip mismatch via {json}");
}

// --- Cards and hands ---

#[test]
fn card_serializes_as_display_string() {
    let json = serde_json::to_string(&c(Suit::Clubs, 7)).unwrap();
    assert_eq!(json, "\"7C\"");
    let json = serde_json::to_string(&c(Suit::Hearts, 10)).unwrap();
    assert_eq!(json, "\"10H\"");
    let json = serde_json::to_string(&c(Suit::Spades, 1)).unwrap();
    assert_eq!(json, "\"1S\"");
}

#[test]
fn card_rejects_invalid_string() {
    assert!(serde_json::from_str::<Card>("\"\"").is_err());
    assert!(serde_json::from_str::<Card>("\"7X\"").is_err());
    assert!(serde_json::from_str::<Card>("\"11H\"").is_err());
    assert!(serde_json::from_str::<Card>("\"0C\"").is_err());
}

#[test]
fn every_card_round_trips() {
    for suit in Suit::ALL {
        for r in 1..=10 {
            roundtrip(c(suit, r));
        }
    }
}

#[test]
fn hand_serializes_as_sorted_card_list() {
    let h = Hand::from(
        [
            c(Suit::Hearts, 2),
            c(Suit::Clubs, 7),
            c(Suit::Diamonds, 1),
            c(Suit::Spades, 10),
        ]
        .as_slice(),
    );
    let json = serde_json::to_string(&h).unwrap();
    // Order follows Card index ordering (suit*10 + rank-1), which is
    // Clubs < Diamonds < Hearts < Spades and ascending rank inside each suit.
    assert_eq!(json, "[\"7C\",\"1D\",\"2H\",\"10S\"]");
    roundtrip(h);
}

#[test]
fn empty_hand_round_trips() {
    roundtrip(Hand::EMPTY);
    assert_eq!(serde_json::to_string(&Hand::EMPTY).unwrap(), "[]");
}

// --- Actions ---

#[test]
fn every_action_variant_round_trips() {
    let seven_c = c(Suit::Clubs, 7);
    let variants = vec![
        Action::DrawStock,
        Action::DrawDiscard,
        Action::Discard(seven_c),
        Action::Close(c(Suit::Hearts, 2)),
        Action::LayMeld(Meld {
            kind: MeldKind::Set,
            cards: Hand::from(
                [c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)].as_slice(),
            ),
        }),
        Action::AddToMeld {
            card: c(Suit::Spades, 7),
            meld_index: 2,
        },
        Action::Pass,
    ];
    for a in variants {
        roundtrip(a);
    }
}

#[test]
fn draw_source_round_trips() {
    roundtrip(DrawSource::Stock);
    roundtrip(DrawSource::TopDiscard);
}

// --- Events ---

#[test]
fn every_event_variant_round_trips() {
    let events = vec![
        Event::Shuffled(vec![c(Suit::Clubs, 1), c(Suit::Spades, 10)]),
        Event::Dealt {
            seat: Seat::new(0),
            hand_size: 7,
        },
        Event::Drew {
            seat: Seat::new(1),
            source: DrawSource::Stock,
        },
        Event::Discarded {
            seat: Seat::new(0),
            card: c(Suit::Hearts, 5),
        },
        Event::Closed {
            seat: Seat::new(1),
            card: c(Suit::Spades, 2),
        },
        Event::LaidMeld {
            seat: Seat::new(0),
            meld: Meld {
                kind: MeldKind::Run,
                cards: Hand::from(
                    [c(Suit::Clubs, 5), c(Suit::Clubs, 6), c(Suit::Clubs, 7)].as_slice(),
                ),
            },
        },
        Event::AddedToMeld {
            seat: Seat::new(1),
            card: c(Suit::Clubs, 8),
            meld_index: 0,
        },
        Event::Passed { seat: Seat::new(0) },
        Event::HandEnded(HandResult {
            points: vec![-10, 12],
            closer: Some(Seat::new(0)),
            bonus: Some(Bonus::MinusTen),
        }),
    ];
    for e in events {
        roundtrip(e);
    }
}

// --- Observation, HandResult, GameLog, RuleSetId, MatchStatus ---

fn sample_observation() -> Observation {
    Observation {
        own_seat: Seat::new(0),
        current_seat: Seat::new(0),
        phase: Phase::Discard,
        own_hand: Hand::from(
            [
                c(Suit::Clubs, 7),
                c(Suit::Hearts, 7),
                c(Suit::Diamonds, 7),
                c(Suit::Spades, 2),
            ]
            .as_slice(),
        ),
        top_discard: Some(c(Suit::Spades, 5)),
        discard_history: vec![c(Suit::Spades, 5)],
        stock_size: 24,
        melds_on_table: vec![Meld {
            kind: MeldKind::Set,
            cards: Hand::from(
                [c(Suit::Clubs, 1), c(Suit::Diamonds, 1), c(Suit::Hearts, 1)].as_slice(),
            ),
        }],
        scores: vec![0, 0],
        hand_sizes: vec![4, 7],
    }
}

#[test]
fn observation_round_trips() {
    roundtrip(sample_observation());
}

#[test]
fn observation_golden_json() {
    let obs = Observation {
        own_seat: Seat::new(0),
        current_seat: Seat::new(1),
        phase: Phase::Draw,
        own_hand: Hand::from([c(Suit::Clubs, 7), c(Suit::Hearts, 2)].as_slice()),
        top_discard: Some(c(Suit::Spades, 5)),
        discard_history: vec![c(Suit::Spades, 5)],
        stock_size: 25,
        melds_on_table: Vec::new(),
        scores: vec![0, 0],
        hand_sizes: vec![2, 7],
    };
    let json = serde_json::to_string(&obs).unwrap();
    let expected = "{\"own_seat\":0,\"current_seat\":1,\"phase\":\"Draw\",\"own_hand\":[\"7C\",\"2H\"],\"top_discard\":\"5S\",\"discard_history\":[\"5S\"],\"stock_size\":25,\"melds_on_table\":[],\"scores\":[0,0],\"hand_sizes\":[2,7]}";
    assert_eq!(json, expected);
}

#[test]
fn hand_result_golden_json() {
    let r = HandResult {
        points: vec![-10, 12, 8],
        closer: Some(Seat::new(0)),
        bonus: Some(Bonus::MinusTen),
    };
    let json = serde_json::to_string(&r).unwrap();
    assert_eq!(
        json,
        "{\"points\":[-10,12,8],\"closer\":0,\"bonus\":\"MinusTen\"}"
    );
    roundtrip(r);
}

#[test]
fn rulesetid_round_trips() {
    let id = RuleSetId {
        name: Cow::Borrowed("conga-standard"),
        version: 1,
        param_hash: 0,
    };
    roundtrip(id);
}

#[test]
fn match_status_variants_round_trip() {
    roundtrip(MatchStatus::InProgress);
    roundtrip(MatchStatus::Winner(Seat::new(2)));
}

#[test]
fn gamelog_round_trips() {
    let log = GameLog {
        ruleset_id: RuleSetId {
            name: Cow::Borrowed("conga-standard"),
            version: 1,
            param_hash: 0,
        },
        seats: 2,
        dealer: Seat::new(1),
        starting_scores: vec![0, 0],
        actions: vec![
            Action::DrawStock,
            Action::Discard(c(Suit::Clubs, 7)),
            Action::Pass,
        ],
        shuffles: vec![vec![c(Suit::Clubs, 1), c(Suit::Spades, 10)]],
    };
    roundtrip(log);
}
