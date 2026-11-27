//! Library side of `conga-cli`. Exposes the subcommand logic so tests
//! can drive it without going through `clap`.

pub mod build_equity;
pub mod debug_hand;
pub mod eval;
pub mod play;
pub mod replay;
