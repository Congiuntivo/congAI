//! `Chance` abstracts every shuffle so the same engine runs under live play
//! and under replay.
//!
//! During a live hand the implementation draws from a seeded RNG. During a
//! replay it reads the next shuffle result from the game log. Either way the
//! engine only sees `shuffle(deck)` and does not care which backing is in
//! use.

use crate::card::Card;

pub trait Chance {
    /// Shuffle `deck` in place. After this call, `deck` contains the cards
    /// in the order they will be dealt or drawn.
    fn shuffle(&mut self, deck: &mut Vec<Card>);
}
