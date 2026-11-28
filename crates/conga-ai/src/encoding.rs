//! Fixed-size seat-relative encoding of (`Observation`, `BeliefTracker`) for
//! the value model (D53).
//!
//! `encode(obs, belief) -> [f32; ENCODING_SIZE]` produces a tensor shape that
//! is the same for 2, 3 and 4 players. Absent seats contribute zeros in their
//! slots so the network reads the same layout regardless of player count.
//! Seat ordering is seat-relative: slot 0 is `own_seat` and slot `i` is
//! `Seat((own_seat.index() + i) % N)` for `i in 0..4`.
//!
//! Layout:
//!
//! | Offset     | Size | Content                                             |
//! | ---------- | ---- | --------------------------------------------------- |
//! | 0..40      | 40   | own-hand bitmask                                    |
//! | 40..80     | 40   | top-discard one-hot (zeros if `None`)               |
//! | 80..120    | 40   | cards-seen bitmask                                  |
//! | 120..124   | 4    | per-seat scores / 100, seat-relative                |
//! | 124..128   | 4    | per-seat hand-sizes / 10, seat-relative             |
//! | 128..168   | 40   | opp[slot 1] `known_held` bitmask (seat-relative)    |
//! | 168..208   | 40   | opp[slot 2] `known_held` bitmask (seat-relative)    |
//! | 208..248   | 40   | opp[slot 3] `known_held` bitmask (seat-relative)    |
//! | 248..251   | 3    | phase one-hot: Draw, Discard, Showdown              |
//! | 251        | 1    | `turn_number / 30` (from `discard_history.len()`)   |
//! | 252        | 1    | `stock_size / 40`                                   |
//! | 253..257   | 4    | seat-present mask, seat-relative                    |
//! | 257..261   | 4    | current-seat one-hot, seat-relative                 |
//!
//! Total: `ENCODING_SIZE = 261` floats. `ENCODING_VERSION = 1`.
//!
//! The cards-seen bitmask is the union of own hand, discard history,
//! melds on the table and every opponent's `known_held` set.

use conga_core::{Hand, Observation, Phase, Seat};

use crate::belief::BeliefTracker;

/// Number of `f32` slots in one encoding. See the module docs for the layout.
pub const ENCODING_SIZE: usize = 261;

/// Version tag; a trained `value_model_v1.json` must match this on load
/// or the loader returns an error (slice 2).
pub const ENCODING_VERSION: u32 = 1;

const MAX_SEATS: usize = 4;

// Sub-block offsets. Private so a future layout bump needs only a
// matching `ENCODING_VERSION` bump plus the loader rejection.
const OFF_OWN_HAND: usize = 0;
const OFF_TOP_DISCARD: usize = 40;
const OFF_CARDS_SEEN: usize = 80;
const OFF_SCORES: usize = 120;
const OFF_HAND_SIZES: usize = 124;
const OFF_KNOWN_HELD: usize = 128; // 3 opponents × 40 cards
const OFF_PHASE: usize = 248;
const OFF_TURN: usize = 251;
const OFF_STOCK: usize = 252;
const OFF_SEAT_PRESENT: usize = 253;
const OFF_CURRENT_SEAT: usize = 257;

/// Encode `(obs, belief)` into a fixed-size seat-relative tensor.
pub fn encode(obs: &Observation, belief: &BeliefTracker) -> [f32; ENCODING_SIZE] {
    debug_assert_eq!(
        belief.own_seat(),
        obs.own_seat,
        "belief tracker seat {:?} != observation seat {:?}",
        belief.own_seat(),
        obs.own_seat
    );
    debug_assert!(
        !matches!(obs.phase, Phase::Ended),
        "encoding is not defined for Phase::Ended",
    );
    debug_assert_eq!(obs.hand_sizes.len(), obs.scores.len());
    debug_assert!(obs.hand_sizes.len() <= MAX_SEATS);

    let mut x = [0.0f32; ENCODING_SIZE];

    write_hand(&mut x[OFF_OWN_HAND..OFF_OWN_HAND + 40], obs.own_hand);

    if let Some(card) = obs.top_discard {
        x[OFF_TOP_DISCARD + card.index() as usize] = 1.0;
    }

    let seen = cards_seen(obs, belief);
    write_hand(&mut x[OFF_CARDS_SEEN..OFF_CARDS_SEEN + 40], seen);

    let own = obs.own_seat.index() as usize;
    let n = obs.hand_sizes.len();
    for i in 0..MAX_SEATS {
        if i >= n {
            continue;
        }
        let source = (own + i) % n;
        x[OFF_SCORES + i] = obs.scores[source] as f32 / 100.0;
        x[OFF_HAND_SIZES + i] = obs.hand_sizes[source] as f32 / 10.0;
        x[OFF_SEAT_PRESENT + i] = 1.0;

        if i > 0 {
            let seat = Seat::new(source as u8);
            if let Some(set) = belief.known_held(seat) {
                let off = OFF_KNOWN_HELD + (i - 1) * 40;
                for &card in set {
                    x[off + card.index() as usize] = 1.0;
                }
            }
        }

        if Seat::new(source as u8) == obs.current_seat {
            x[OFF_CURRENT_SEAT + i] = 1.0;
        }
    }

    match obs.phase {
        Phase::Draw => x[OFF_PHASE] = 1.0,
        Phase::Discard => x[OFF_PHASE + 1] = 1.0,
        Phase::Showdown => x[OFF_PHASE + 2] = 1.0,
        Phase::Ended => {}
    }

    x[OFF_TURN] = obs.discard_history.len() as f32 / 30.0;
    x[OFF_STOCK] = obs.stock_size as f32 / 40.0;

    x
}

fn write_hand(slice: &mut [f32], hand: Hand) {
    for card in hand.iter() {
        slice[card.index() as usize] = 1.0;
    }
}

fn cards_seen(obs: &Observation, belief: &BeliefTracker) -> Hand {
    let mut seen = obs.own_hand;
    for &c in &obs.discard_history {
        seen.insert(c);
    }
    for meld in &obs.melds_on_table {
        seen = seen.union(meld.cards);
    }
    for seat_idx in 0..belief.seats_count() {
        let seat = Seat::new(seat_idx);
        if seat == obs.own_seat {
            continue;
        }
        if let Some(set) = belief.known_held(seat) {
            for &c in set {
                seen.insert(c);
            }
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use conga_core::{Card, DrawSource, Event, Meld, MeldKind, Rank, Suit};

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    fn two_seat_obs(own_seat: u8, own_hand: Hand) -> Observation {
        Observation {
            own_seat: Seat::new(own_seat),
            current_seat: Seat::new(own_seat),
            phase: Phase::Draw,
            own_hand,
            top_discard: None,
            discard_history: Vec::new(),
            stock_size: 0,
            melds_on_table: Vec::new(),
            scores: vec![0, 0],
            hand_sizes: vec![own_hand.len() as u8, 0],
        }
    }

    #[test]
    fn encoding_size_is_261() {
        assert_eq!(ENCODING_SIZE, 261);
    }

    #[test]
    fn own_hand_bits_set() {
        let hand: Hand = [
            c(Suit::Clubs, 1),
            c(Suit::Clubs, 2),
            c(Suit::Clubs, 3),
            c(Suit::Hearts, 7),
            c(Suit::Hearts, 8),
            c(Suit::Hearts, 9),
            c(Suit::Spades, 10),
        ]
        .as_slice()
        .into();
        let obs = two_seat_obs(0, hand);
        let belief = BeliefTracker::new(Seat::new(0), 2);
        let x = encode(&obs, &belief);

        for card in hand.iter() {
            assert_eq!(x[OFF_OWN_HAND + card.index() as usize], 1.0);
        }
        let set_count: usize = x[OFF_OWN_HAND..OFF_OWN_HAND + 40]
            .iter()
            .filter(|&&v| v == 1.0)
            .count();
        assert_eq!(set_count, hand.len() as usize);
    }

    #[test]
    fn top_discard_none_is_zero_block() {
        let obs = two_seat_obs(0, Hand::EMPTY);
        let belief = BeliefTracker::new(Seat::new(0), 2);
        let x = encode(&obs, &belief);
        assert!(
            x[OFF_TOP_DISCARD..OFF_TOP_DISCARD + 40]
                .iter()
                .all(|&v| v == 0.0)
        );
    }

    #[test]
    fn top_discard_some_is_one_hot() {
        let mut obs = two_seat_obs(0, Hand::EMPTY);
        let card = c(Suit::Hearts, 7);
        obs.top_discard = Some(card);
        obs.discard_history = vec![card];
        let belief = BeliefTracker::new(Seat::new(0), 2);
        let x = encode(&obs, &belief);
        let set: Vec<usize> = x[OFF_TOP_DISCARD..OFF_TOP_DISCARD + 40]
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (*v == 1.0).then_some(i))
            .collect();
        assert_eq!(set, vec![card.index() as usize]);
    }

    #[test]
    fn phase_one_hot_sums_to_one() {
        for (phase, expected_slot) in [(Phase::Draw, 0), (Phase::Discard, 1), (Phase::Showdown, 2)]
        {
            let mut obs = two_seat_obs(0, Hand::EMPTY);
            obs.phase = phase;
            let belief = BeliefTracker::new(Seat::new(0), 2);
            let x = encode(&obs, &belief);
            let block = &x[OFF_PHASE..OFF_PHASE + 3];
            let sum: f32 = block.iter().sum();
            assert_eq!(sum, 1.0);
            assert_eq!(block[expected_slot], 1.0);
        }
    }

    #[test]
    fn cards_seen_unions_hand_discard_melds_and_belief() {
        let own_hand: Hand = [c(Suit::Clubs, 1), c(Suit::Clubs, 2)].as_slice().into();
        let mut obs = two_seat_obs(0, own_hand);
        obs.discard_history = vec![c(Suit::Clubs, 3)];
        obs.melds_on_table = vec![Meld {
            kind: MeldKind::Set,
            cards: [c(Suit::Clubs, 5), c(Suit::Diamonds, 5), c(Suit::Hearts, 5)]
                .as_slice()
                .into(),
        }];
        obs.hand_sizes = vec![2, 7];

        let mut belief = BeliefTracker::new(Seat::new(0), 2);
        // Put 4C into seat 1's known_held via the Discard + TopDiscard path.
        belief.notify(&Event::Discarded {
            seat: Seat::new(0),
            card: c(Suit::Clubs, 4),
        });
        belief.notify(&Event::Drew {
            seat: Seat::new(1),
            source: DrawSource::TopDiscard,
        });
        assert!(
            belief
                .known_held(Seat::new(1))
                .unwrap()
                .contains(&c(Suit::Clubs, 4))
        );

        let x = encode(&obs, &belief);
        let expected_seen = [
            c(Suit::Clubs, 1),
            c(Suit::Clubs, 2),
            c(Suit::Clubs, 3),
            c(Suit::Clubs, 4),
            c(Suit::Clubs, 5),
            c(Suit::Diamonds, 5),
            c(Suit::Hearts, 5),
        ];
        let set: Vec<usize> = x[OFF_CARDS_SEEN..OFF_CARDS_SEEN + 40]
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (*v == 1.0).then_some(i))
            .collect();
        let mut expected_indices: Vec<usize> =
            expected_seen.iter().map(|c| c.index() as usize).collect();
        expected_indices.sort_unstable();
        assert_eq!(set, expected_indices);
    }

    #[test]
    fn seat_relative_scores_and_hand_sizes_are_cyclically_rotated() {
        let scores = vec![10, 20, 30, 40];
        let hand_sizes = vec![6, 7, 8, 5];
        for own_idx in 0..4u8 {
            let obs = Observation {
                own_seat: Seat::new(own_idx),
                current_seat: Seat::new(0),
                phase: Phase::Draw,
                own_hand: Hand::EMPTY,
                top_discard: None,
                discard_history: Vec::new(),
                stock_size: 10,
                melds_on_table: Vec::new(),
                scores: scores.clone(),
                hand_sizes: hand_sizes.clone(),
            };
            let belief = BeliefTracker::new(Seat::new(own_idx), 4);
            let x = encode(&obs, &belief);
            for i in 0..4 {
                let source = ((own_idx as usize) + i) % 4;
                assert!(
                    (x[OFF_SCORES + i] - scores[source] as f32 / 100.0).abs() < 1e-6,
                    "own={own_idx} slot={i} source={source}"
                );
                assert!(
                    (x[OFF_HAND_SIZES + i] - hand_sizes[source] as f32 / 10.0).abs() < 1e-6,
                    "own={own_idx} slot={i} source={source}"
                );
                assert_eq!(x[OFF_SEAT_PRESENT + i], 1.0);
            }
        }
    }

    #[test]
    fn absent_seats_in_two_player_are_zero() {
        let obs = two_seat_obs(0, Hand::EMPTY);
        let belief = BeliefTracker::new(Seat::new(0), 2);
        let x = encode(&obs, &belief);
        // Slots 2 and 3 must be zero across every per-seat block.
        for i in 2..4 {
            assert_eq!(x[OFF_SCORES + i], 0.0);
            assert_eq!(x[OFF_HAND_SIZES + i], 0.0);
            assert_eq!(x[OFF_SEAT_PRESENT + i], 0.0);
            assert_eq!(x[OFF_CURRENT_SEAT + i], 0.0);
        }
        // Opponent slot 2 and 3 known_held bitmasks (offsets +40 and +80)
        // are zero; slot 1 is for the only opponent.
        for off in [OFF_KNOWN_HELD + 40, OFF_KNOWN_HELD + 80] {
            assert!(x[off..off + 40].iter().all(|&v| v == 0.0));
        }
    }

    #[test]
    fn current_seat_one_hot_marks_the_actor() {
        let mut obs = two_seat_obs(0, Hand::EMPTY);
        obs.hand_sizes = vec![7, 7];
        let belief = BeliefTracker::new(Seat::new(0), 2);

        // Own turn -> slot 0 is 1.
        obs.current_seat = Seat::new(0);
        let x = encode(&obs, &belief);
        assert_eq!(x[OFF_CURRENT_SEAT], 1.0);
        assert_eq!(x[OFF_CURRENT_SEAT + 1], 0.0);

        // Opponent's turn -> slot 1 is 1.
        obs.current_seat = Seat::new(1);
        let x = encode(&obs, &belief);
        assert_eq!(x[OFF_CURRENT_SEAT], 0.0);
        assert_eq!(x[OFF_CURRENT_SEAT + 1], 1.0);
    }

    #[test]
    fn opponent_known_held_lands_in_slot_1_for_two_player() {
        let mut obs = two_seat_obs(0, Hand::EMPTY);
        obs.hand_sizes = vec![7, 7];
        let mut belief = BeliefTracker::new(Seat::new(0), 2);
        let known = c(Suit::Spades, 9);
        belief.notify(&Event::Discarded {
            seat: Seat::new(0),
            card: known,
        });
        belief.notify(&Event::Drew {
            seat: Seat::new(1),
            source: DrawSource::TopDiscard,
        });

        let x = encode(&obs, &belief);
        // Slot 1 opponent (seat 1): bit set for 9S.
        assert_eq!(x[OFF_KNOWN_HELD + known.index() as usize], 1.0);
        // Slots 2 and 3 untouched.
        for off in [OFF_KNOWN_HELD + 40, OFF_KNOWN_HELD + 80] {
            assert!(x[off..off + 40].iter().all(|&v| v == 0.0));
        }
    }

    #[test]
    fn turn_and_stock_scale_to_units() {
        let mut obs = two_seat_obs(0, Hand::EMPTY);
        obs.discard_history = vec![c(Suit::Clubs, 1); 15]; // 15 / 30 = 0.5
        obs.stock_size = 20; // 20 / 40 = 0.5
        let belief = BeliefTracker::new(Seat::new(0), 2);
        let x = encode(&obs, &belief);
        assert!((x[OFF_TURN] - 0.5).abs() < 1e-6);
        assert!((x[OFF_STOCK] - 0.5).abs() < 1e-6);
    }
}
