//! Agents.
//!
//! m2 shipped the `Player` trait, a `RandomPlayer` and a `run_player`
//! driver. m3 adds the baseline agents (greedy, rule-based) and the
//! evaluation harness. The belief tracker and search agent arrive in
//! m4. m5 adds the 2-player match equity table (D39-D42) and will
//! layer a score-aware agent on top of it.

pub mod belief;
pub mod driver;
pub mod equity;
pub mod eval;
pub mod features;
pub mod greedy;
pub mod player;
pub mod random;
pub mod rule_based;
pub mod score_aware;
pub mod search;

pub use belief::BeliefTracker;
pub use driver::{DriverError, run_player};
pub use equity::{EquityTable, committed_table};
pub use eval::{
    BaselineAgent, EvalReport, MatchOutcome, PairedResult, report_from_pairs, run_batch, run_match,
    run_match_from, run_paired, wilson_ci,
};
pub use features::{Features, extract as extract_features};
pub use greedy::GreedyPlayer;
pub use player::Player;
pub use random::RandomPlayer;
pub use rule_based::RuleBasedPlayer;
pub use score_aware::{DEFAULT_SCORE_AWARE_SAMPLES, ScoreAwarePlayer};
pub use search::{RolloutPolicy, SearchPlayer};
