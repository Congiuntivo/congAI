//! `RuleBasedPlayer` behaviour on canned positions.

use conga_ai::{GreedyPlayer, Player, RuleBasedPlayer};
use conga_core::{
    Action, Card, DrawSource, Event, Hand, Meld, MeldKind, Observation, Phase, Rank, Seat, Suit,
};

fn c(suit: Suit, rank: u8) -> Card {
    Card::new(suit, Rank::new(rank).unwrap())
}

fn obs(hand: &[Card], melds: Vec<Meld>, top: Option<Card>) -> Observation {
    Observation {
        own_seat: Seat::new(0),
        current_seat: Seat::new(0),
        phase: Phase::Discard,
        own_hand: Hand::from(hand),
        top_discard: top,
        discard_history: top.into_iter().collect(),
        stock_size: 25,
        melds_on_table: melds,
        scores: vec![0, 0],
        hand_sizes: vec![hand.len() as u8, 7],
    }
}

#[test]
fn mirrors_greedy_when_the_safety_signal_is_silent() {
    // No melds on the table, no opponent pickups observed: rule-based
    // is pointwise identical to greedy on any observation.
    let mut rb = RuleBasedPlayer::new();
    let mut gr = GreedyPlayer::new();
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
    let o = obs(&hand, Vec::new(), None);
    let legal: Vec<Action> = hand.iter().map(|card| Action::Discard(*card)).collect();
    assert_eq!(rb.act(&o, &legal), gr.act(&o, &legal));
}

#[test]
fn refuses_to_feed_a_visible_set_when_a_safer_tie_exists() {
    // The next seat has {5♣ 5♦ 5♥} on the table (a Set of 5s). I have
    // two unmatched 10s (10♣, 10♥) on top of two melds — oh wait,
    // greedy's pick is highest rank, so greedy would discard a 10.
    // Add a 5♠ to the legal set to make it the greedy-tied pair:
    // two unmatched 5s where one extends the Set and one does not.
    // Simpler test: three 10s on the table; I have 10♠ and 10 of
    // something else. Only 10 of the four exists if three are visible.
    // So use a Set of 5s and make sure *my* highest unmatched rank
    // matches — I'll stuff my hand so that greedy picks a 5.
    //
    // Hand: {1♣ 2♣ 3♣} (Run) + {5♣, 5♠}. With 5♣ 5♦ 5♥ on the table,
    // 5♣ cannot be in my hand (same card already placed). Scratch.
    //
    // Cleanest: Set of 5s on table using suits {Clubs, Diamonds, Hearts}.
    // My hand has {5♠, 7♠} unmatched. Greedy picks 7♠ (higher rank).
    // No tie, no safety fire. Not this case.
    //
    // For the safety filter to fire we need a greedy tie by rank.
    // Build a hand with two cards of the same highest unmatched rank
    // where one of them extends the visible meld.
    //
    // Set of 2s on table: {2♣ 2♦ 2♥}. Hand: {2♠, 7♣, 7♦, 7♥} (meld)
    // plus two more unmatched 10s: {10♣, 10♠}. Greedy picks 10♣
    // (lower index), safety is silent (neither 10 extends a 2-set),
    // so rule-based agrees. Not this case either.
    //
    // To get safety to actually change the pick: two tied-rank
    // candidates where only one extends. Need: meld of R on table +
    // two cards of rank R' where R' == R (then only ones that aren't
    // already on-table extend). But a Set of 3 of rank R already uses
    // 3 of the 4 suits, so I can only hold 1 of that rank (the 4th).
    // That gives one card, no tie.
    //
    // Workable: Run on table and two tied-rank cards on either side of
    // its range. Run {5♠ 6♠ 7♠}. Extensions: 4♠ or 8♠. My hand has
    // two unmatched 8s: 8♠ (extends) and 8♥ (does not). Greedy picks
    // the highest rank; if 8 is my highest unmatched, greedy picks 8♠
    // (lower index) or 8♥ depending on ordering — but my greedy
    // breaks ties by lower card index, so 8♠ (index 37) vs 8♥
    // (index 27): 8♥ has the lower index, greedy picks 8♥. 8♥ is
    // the SAFE one. Rule-based picks the same. Hmm — that passes but
    // doesn't prove rule-based diverges.
    //
    // Flip the Run to one in Hearts: {5♥ 6♥ 7♥}. Extensions: 4♥ or
    // 8♥. Unmatched 8s: 8♣ (index 7) and 8♥ (index 27). Greedy picks
    // 8♣ (lower index) — both safe, no change. Still no divergence.
    //
    // Trick: make greedy's tiebreaker pick the UNSAFE one. We need
    // the unsafe card to win greedy's tiebreak (lower card index)
    // and rule-based to pick the other. Run in Clubs {5♣ 6♣ 7♣};
    // unmatched 8s: 8♣ (index 7, unsafe, extends the run) vs 8♦
    // (index 17, safe). Greedy picks 8♣. Rule-based should pick 8♦.
    let mut rb = RuleBasedPlayer::new();
    let run = Meld {
        kind: MeldKind::Run,
        cards: Hand::from(&[c(Suit::Clubs, 5), c(Suit::Clubs, 6), c(Suit::Clubs, 7)][..]),
    };
    // Hand: three 2s (meld of three) + two 8s unmatched + one filler.
    let hand = [
        c(Suit::Spades, 2),
        c(Suit::Diamonds, 2),
        c(Suit::Hearts, 2),
        c(Suit::Clubs, 8),
        c(Suit::Diamonds, 8),
        c(Suit::Spades, 4),
    ];
    let o = obs(&hand, vec![run], None);
    let legal = vec![
        Action::Discard(c(Suit::Clubs, 8)),
        Action::Discard(c(Suit::Diamonds, 8)),
    ];

    // Sanity: greedy picks the unsafe one (lower card index).
    let mut gr = GreedyPlayer::new();
    assert_eq!(
        gr.act(&o, &legal),
        Action::Discard(c(Suit::Clubs, 8)),
        "greedy baseline picks 8♣ by index tiebreak"
    );

    // Rule-based swaps to the safe 8♦.
    assert_eq!(
        rb.act(&o, &legal),
        Action::Discard(c(Suit::Diamonds, 8)),
        "rule-based prefers 8♦ because 8♣ would extend the visible run"
    );
}

#[test]
fn refuses_to_feed_a_card_the_next_seat_took_from_discard() {
    // Opponent (seat 1) took a 10♠ from the discard pile a turn ago.
    // That suggests they're building a Set of 10s. I now have two
    // tied unmatched cards — 10♥ (same rank, risky) and 10♦ (also
    // same rank, also risky, but...actually both 10s are risky under
    // the rank signal). Let me instead use a Run signal: opponent
    // took 6♣ (adjacent to my 7♣ and 5♣). Hand has two tied-rank
    // candidates where only one shares the risky suit.
    let mut rb = RuleBasedPlayer::new();

    // Train rule-based: opponent discarded 6♣, then next seat drew
    // from the top of discard (picked it up).
    rb.notify(&Event::Discarded {
        seat: Seat::new(0),
        card: c(Suit::Clubs, 6),
    });
    rb.notify(&Event::Drew {
        seat: Seat::new(1),
        source: DrawSource::TopDiscard,
    });

    // Hand: run of 1-3 in Hearts (meld) + two tied-rank 7s
    // (7♣ risky because the opponent is building clubs; 7♦ safe).
    let hand = [
        c(Suit::Hearts, 1),
        c(Suit::Hearts, 2),
        c(Suit::Hearts, 3),
        c(Suit::Clubs, 7),
        c(Suit::Diamonds, 7),
        c(Suit::Spades, 4),
    ];
    let o = obs(&hand, Vec::new(), None);
    let legal = vec![
        Action::Discard(c(Suit::Clubs, 7)),
        Action::Discard(c(Suit::Diamonds, 7)),
    ];

    // Greedy would pick 7♣ (lower card index).
    let mut gr = GreedyPlayer::new();
    assert_eq!(gr.act(&o, &legal), Action::Discard(c(Suit::Clubs, 7)));

    // Rule-based swaps to 7♦.
    assert_eq!(rb.act(&o, &legal), Action::Discard(c(Suit::Diamonds, 7)));
}

#[test]
fn hand_ended_resets_opponent_pickups() {
    let mut rb = RuleBasedPlayer::new();
    rb.notify(&Event::Discarded {
        seat: Seat::new(0),
        card: c(Suit::Clubs, 6),
    });
    rb.notify(&Event::Drew {
        seat: Seat::new(1),
        source: DrawSource::TopDiscard,
    });
    rb.notify(&Event::HandEnded(conga_core::HandResult {
        points: vec![0, 0],
        closer: None,
        bonus: None,
        terminal: conga_core::HandTerminal::Closed,
    }));

    // After HandEnded the memory of seat-1 picking up 6♣ is gone, so
    // 7♣ is no longer flagged, and rule-based mirrors greedy again.
    let hand = [
        c(Suit::Hearts, 1),
        c(Suit::Hearts, 2),
        c(Suit::Hearts, 3),
        c(Suit::Clubs, 7),
        c(Suit::Diamonds, 7),
        c(Suit::Spades, 4),
    ];
    let o = obs(&hand, Vec::new(), None);
    let legal = vec![
        Action::Discard(c(Suit::Clubs, 7)),
        Action::Discard(c(Suit::Diamonds, 7)),
    ];
    let mut gr = GreedyPlayer::new();
    assert_eq!(rb.act(&o, &legal), gr.act(&o, &legal));
}

#[test]
fn delegates_non_discard_decisions_to_greedy() {
    // A Close is legal; rule-based doesn't touch non-discard
    // decisions, so it should take the Close that greedy picks.
    let mut rb = RuleBasedPlayer::new();
    let hand = [c(Suit::Clubs, 2)];
    let o = obs(&hand, Vec::new(), None);
    let legal = vec![
        Action::DrawStock,
        Action::Discard(c(Suit::Clubs, 2)),
        Action::Close(c(Suit::Clubs, 2)),
        Action::Pass,
    ];
    let a = rb.act(&o, &legal);
    assert!(matches!(a, Action::Close(_)), "got {a:?}");
}
