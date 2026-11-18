//! `RandomPlayer`: picks uniformly at random from `legal`.
//!
//! Uses ChaCha8 as the RNG so every test is reproducible from a seed.
//! The engine's `RandomChance` uses the same family, but each has its
//! own stream — a match's random seed and a player's random seed do not
//! interact.

use conga_core::{Action, Observation};
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

use crate::player::Player;

pub struct RandomPlayer<R: Rng + Send> {
    rng: R,
}

impl RandomPlayer<ChaCha8Rng> {
    pub fn from_seed(seed: u64) -> Self {
        RandomPlayer {
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}

impl<R: Rng + Send> RandomPlayer<R> {
    pub fn new(rng: R) -> Self {
        RandomPlayer { rng }
    }
}

impl<R: Rng + Send> Player for RandomPlayer<R> {
    fn act(&mut self, _obs: &Observation, legal: &[Action]) -> Action {
        assert!(
            !legal.is_empty(),
            "RandomPlayer::act called with no legal actions"
        );
        let idx = (self.rng.next_u32() as usize) % legal.len();
        legal[idx].clone()
    }
}
