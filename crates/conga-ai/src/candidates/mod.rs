//! Candidate tactics (D47). Each candidate lives in its own file and
//! implements [`Player`](crate::player::Player) by wrapping an inner
//! player, overriding only the actions the tactic targets. Candidates
//! do not go on the `BaselineAgent` ladder until they pass the
//! [`validate_tactic`](crate::validation::validate_tactic) loop.

pub mod clean_split_close;

pub use clean_split_close::{CleanSplitCloseWrapper, DEFAULT_CLEAN_SPLIT_SAMPLES};
