//! `RuleBasedPlayer`: greedy + discard-safety v1.
//!
//! Per D28: scores each legal discard that is tied with greedy's choice
//! by how likely the next seat can use it. Signals the agent reads:
//! - Visible melds on the table (`obs.melds_on_table`) that the card
//!   could extend. During the main playing phase `melds_on_table` is
//!   empty in StandardConga so this signal mostly matters in showdown.
//! - Cards the next seat has picked up from the discard pile this hand
//!   (tracked via `notify`): same rank suggests a Set in progress, an
//!   adjacent same-suit rank suggests a Run.
//!
//! Among greedy-tied discards it picks the lowest-risk card; ties
//! break by card index for determinism. When no discard is "safe"
//! the agent falls back to greedy's choice.

use std::collections::HashMap;

use conga_core::{Action, Card, DrawSource, Event, Meld, MeldKind, Observation};

use crate::greedy::GreedyPlayer;
use crate::player::Player;

#[derive(Debug, Default, Clone)]
pub struct RuleBasedPlayer {
    greedy: GreedyPlayer,
    /// Per-seat cards that seat picked up from the top of the discard
    /// pile this hand. Reset on `HandEnded`.
    opponent_pickups: HashMap<u8, Vec<Card>>,
    /// The last card broadcast as `Event::Discarded`. The next
    /// `Event::Drew { source: TopDiscard }` to arrive before any other
    /// `Discarded` describes that card; later TopDiscard draws peel
    /// off an unknown card below (we clear this field and wait for the
    /// next `Observe` to learn the new top).
    last_discarded: Option<Card>,
}

impl RuleBasedPlayer {
    pub fn new() -> Self {
        RuleBasedPlayer::default()
    }

    fn risk(&self, card: Card, obs: &Observation, next_seat: u8) -> u32 {
        let mut score = 0u32;
        for meld in &obs.melds_on_table {
            if extends(meld, card) {
                score += 10;
            }
        }
        if let Some(pickups) = self.opponent_pickups.get(&next_seat) {
            for p in pickups {
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

        let next_seat = next_seat_after(obs);
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
        match event {
            Event::Discarded { card, .. } => {
                self.last_discarded = Some(*card);
            }
            Event::Drew {
                seat,
                source: DrawSource::TopDiscard,
            } => {
                if let Some(card) = self.last_discarded {
                    self.opponent_pickups
                        .entry(seat.index())
                        .or_default()
                        .push(card);
                }
                self.last_discarded = None;
            }
            Event::HandEnded(_) => {
                self.opponent_pickups.clear();
                self.last_discarded = None;
            }
            _ => {}
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
