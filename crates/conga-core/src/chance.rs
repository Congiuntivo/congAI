//! `Chance` abstracts every shuffle so the same engine runs under live play
//! and under replay.
//!
//! During a live hand `RandomChance` draws from a seeded RNG and records
//! each shuffle result. During a replay `ReplayChance` feeds those same
//! results back in order. Either way the engine only sees `shuffle(deck)`
//! and does not care which backing is in use.

use std::collections::VecDeque;

use rand_core::Rng;

use crate::card::Card;

pub trait Chance {
    /// Shuffle `deck` in place. After this call, `deck` contains the cards
    /// in the order they will be dealt or drawn.
    fn shuffle(&mut self, deck: &mut Vec<Card>);
}

/// Shuffles via a seeded RNG and records every shuffle result so the hand
/// can later be replayed. Fisher-Yates with the simple `next_u32 % n`
/// mapping: the small bias is acceptable because the shuffle output is
/// logged and reproducibility matters more than exact uniformity.
pub struct RandomChance<R: Rng> {
    pub rng: R,
    pub shuffles: Vec<Vec<Card>>,
}

impl<R: Rng> RandomChance<R> {
    pub fn new(rng: R) -> Self {
        RandomChance {
            rng,
            shuffles: Vec::new(),
        }
    }

    pub fn into_shuffles(self) -> Vec<Vec<Card>> {
        self.shuffles
    }
}

impl<R: Rng> Chance for RandomChance<R> {
    fn shuffle(&mut self, deck: &mut Vec<Card>) {
        for i in (1..deck.len()).rev() {
            let j = (self.rng.next_u32() as usize) % (i + 1);
            deck.swap(i, j);
        }
        self.shuffles.push(deck.clone());
    }
}

/// Reads shuffle results from a log and overwrites each `deck` with the
/// next recorded result, so apply() takes the same path it took on the
/// original run.
pub struct ReplayChance {
    pub shuffles: VecDeque<Vec<Card>>,
}

impl ReplayChance {
    pub fn new(shuffles: Vec<Vec<Card>>) -> Self {
        ReplayChance {
            shuffles: shuffles.into(),
        }
    }
}

impl Chance for ReplayChance {
    fn shuffle(&mut self, deck: &mut Vec<Card>) {
        match self.shuffles.pop_front() {
            Some(logged) => {
                *deck = logged;
            }
            None => panic!("ReplayChance exhausted: no more shuffles in the log"),
        }
    }
}
