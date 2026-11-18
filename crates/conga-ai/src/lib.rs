//! Agents.
//!
//! m2 shipped the `Player` trait, a `RandomPlayer` and a `run_player`
//! driver. m3 adds the baseline agents (greedy, rule-based) and the
//! evaluation harness. The belief tracker and search agent arrive in
//! m4.

pub mod driver;
pub mod greedy;
pub mod player;
pub mod random;
pub mod rule_based;

pub use driver::{DriverError, run_player};
pub use greedy::GreedyPlayer;
pub use player::Player;
pub use random::RandomPlayer;
pub use rule_based::RuleBasedPlayer;
