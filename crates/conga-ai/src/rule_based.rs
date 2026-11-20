//! `RuleBasedPlayer` v2: greedy + belief-informed discard safety.
//!
//! Replaces D28's ad-hoc `opponent_pickups` tracking with the
//! `BeliefTracker` (D32). Signals:
//! - Visible melds on the table (`obs.melds_on_table`) that the card
//!   could extend. During the main playing phase this is empty in
//!   StandardConga, so this mostly matters in showdown.
//! - Cards the belief tracker infers the next seat currently holds
//!   (from Drew{TopDiscard} chains, minus LaidMeld / AddedToMeld /
//!   Closed removals). Strictly more accurate than D28's monotonic
//!   pickup list, at the cost of forgetting cards the opponent has
//!   since discarded.
//!
//! Among greedy-tied discards it picks the lowest-risk card; ties
//! break by card index for determinism. When no discard is "safe" the
//! agent falls back to greedy. Non-discard decisions pass through to
//! greedy unchanged.

use conga_core::{Action, Card, Event, Meld, MeldKind, Observation, Seat};

use crate::belief::BeliefTracker;
use crate::greedy::GreedyPlayer;
use crate::player::Player;

#[derive(Debug, Default, Clone)]
pub struct RuleBasedPlayer {
    greedy: GreedyPlayer,
    /// Lazy-initialised on the first `act` call once we know our own
    /// seat and the seat count. Events received before that are
    /// buffered into `pending_events` and replayed on init.
    belief: Option<BeliefTracker>,
    pending_events: Vec<Event>,
}

impl RuleBasedPlayer {
    pub fn new() -> Self {
        RuleBasedPlayer::default()
    }

    fn ensure_belief(&mut self, obs: &Observation) {
        if self.belief.is_none() {
            let mut b = BeliefTracker::new(obs.own_seat, obs.hand_sizes.len() as u8);
            for e in self.pending_events.drain(..) {
                b.notify(&e);
            }
            self.belief = Some(b);
        }
    }

    fn risk(&self, card: Card, obs: &Observation, next_seat: Seat) -> u32 {
        let mut score = 0u32;
        for meld in &obs.melds_on_table {
            if extends(meld, card) {
                score += 10;
            }
        }
        if let Some(belief) = &self.belief
            && let Some(known) = belief.known_held(next_seat)
        {
            for p in known {
                if p.rank() == card.rank() && *p != card {
                    score += 5;
                }
                let dr = (p.rank().as_u8() as i16 - card.rank().as_u8() as i16).abs();
                if p.suit() == card.suit() && (1..=2).contains(&dr) {
                    score += 3;
                }
            }
        }
        score
    }
}

impl Player for RuleBasedPlayer {
    fn act(&mut self, obs: &Observation, legal: &[Action]) -> Action {
        self.ensure_belief(obs);
        let greedy_action = self.greedy.act(obs, legal);
        let Action::Discard(greedy_card) = &greedy_action else {
            return greedy_action;
        };
        let greedy_card = *greedy_card;

        let candidates: Vec<Card> = legal
            .iter()
            .filter_map(|a| {
                if let Action::Discard(c) = a {
                    Some(*c)
                } else {
                    None
                }
            })
            .filter(|c| c.rank() == greedy_card.rank())
            .collect();

        if candidates.len() <= 1 {
            return Action::Discard(greedy_card);
        }

        let next_seat = Seat::new(next_seat_after(obs));
        let safest = candidates
            .iter()
            .copied()
            .min_by(|a, b| {
                self.risk(*a, obs, next_seat)
                    .cmp(&self.risk(*b, obs, next_seat))
                    .then(a.index().cmp(&b.index()))
            })
            .unwrap_or(greedy_card);
        Action::Discard(safest)
    }

    fn notify(&mut self, event: &Event) {
        match &mut self.belief {
            Some(b) => b.notify(event),
            None => self.pending_events.push(event.clone()),
        }
    }
}

fn next_seat_after(obs: &Observation) -> u8 {
    let seats = obs.hand_sizes.len() as u8;
    if seats == 0 {
        0
    } else {
        (obs.own_seat.index() + 1) % seats
    }
}

fn extends(meld: &Meld, card: Card) -> bool {
    if meld.cards.contains(card) {
        return false;
    }
    match meld.kind {
        MeldKind::Set => meld
            .cards
            .iter()
            .next()
            .is_some_and(|any| any.rank() == card.rank()),
        MeldKind::Run => {
            let cards: Vec<Card> = meld.cards.iter().collect();
            let Some(first) = cards.first() else {
                return false;
            };
            if card.suit() != first.suit() {
                return false;
            }
            let (min, max) = cards.iter().fold((10u8, 1u8), |(lo, hi), c| {
                let r = c.rank().as_u8();
                (lo.min(r), hi.max(r))
            });
            let r = card.rank().as_u8();
            (min >= 1 && r + 1 == min) || r == max + 1
        }
    }
}
