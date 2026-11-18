//! Agents.
//!
//! m2 ships only the bare minimum the exit test needs: the `Player`
//! trait, a `RandomPlayer` that picks uniformly from `legal`, and a
//! `run_player` driver that bridges a `Player` to a `Transport`. The
//! greedy and rule-based baselines plus the evaluation harness arrive
//! in m3; the belief tracker and search agent in m4.

pub mod driver;
pub mod player;
pub mod random;

pub use driver::{DriverError, run_player};
pub use player::Player;
pub use random::RandomPlayer;
