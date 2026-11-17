//! Terminal client for playing a match.
//!
//! m2 ships one screen: the acting seat's observation plus a numbered
//! menu of legal actions. Local multiplayer with a hand-over screen is
//! deferred until a later milestone needs it (D20). Nothing in here
//! holds game state beyond the server's last `Observation`; the server
//! is still the only authoritative source.

pub mod app;
pub mod driver;
pub mod input;
pub mod render;

pub use app::AppState;
pub use driver::{TuiError, run_tui};
pub use input::{KeyIntent, key_to_intent};
pub use render::render_hand;
