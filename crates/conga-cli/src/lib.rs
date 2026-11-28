//! Library side of `conga-cli`. Exposes the subcommand logic so tests
//! can drive it without going through `clap`.

pub mod build_equity;
pub mod counterfactual_sweep;
pub mod debug_hand;
pub mod eval;
pub mod mine_disagreements;
pub mod play;
pub mod replay;
pub mod self_play_batch;
pub mod validate_tactic;
