//! Client-side driver: bridges a `Player` to a `Transport`.
//!
//! Reads server messages, feeds the latest `Observation` to the player
//! whenever an `AwaitAction` arrives, sends the player's choice back as
//! an `Act`, and exits cleanly on `MatchEnded`.

use std::fmt;

use conga_core::Observation;
use conga_protocol::{ClientToServer, ServerToClient};
use conga_server::{Transport, TransportError};

use crate::player::Player;

#[derive(Debug)]
pub enum DriverError {
    Transport(TransportError),
    /// The server sent an `AwaitAction` before any `Observe`, so the
    /// player has no observation to decide from. Fatal because every
    /// server loop in m2 sends `Observe` first.
    AwaitWithoutObserve,
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DriverError::Transport(e) => write!(f, "transport: {e}"),
            DriverError::AwaitWithoutObserve => {
                f.write_str("server prompted for an action before sending an Observation")
            }
        }
    }
}

impl std::error::Error for DriverError {}

impl From<TransportError> for DriverError {
    fn from(value: TransportError) -> Self {
        DriverError::Transport(value)
    }
}

/// Drive one Player over a Transport until the match ends.
pub async fn run_player<P, T>(mut player: P, mut transport: T) -> Result<(), DriverError>
where
    P: Player,
    T: Transport<Outgoing = ClientToServer, Incoming = ServerToClient> + Send,
{
    let mut latest_turn: u32 = 0;
    let mut latest_obs: Option<Observation> = None;
    loop {
        let msg = transport.recv().await?;
        match msg {
            ServerToClient::Observe { turn, observation } => {
                latest_turn = turn;
                latest_obs = Some(observation);
            }
            ServerToClient::AwaitAction { legal, .. } => {
                let obs = latest_obs
                    .as_ref()
                    .ok_or(DriverError::AwaitWithoutObserve)?;
                let action = player.act(obs, &legal);
                transport
                    .send(ClientToServer::Act {
                        turn: latest_turn,
                        action,
                    })
                    .await?;
            }
            ServerToClient::Event { event, .. } => {
                player.notify(&event);
            }
            ServerToClient::HandEnded { .. } => {
                // A new hand starts fresh with its own Observe.
                latest_obs = None;
            }
            ServerToClient::MatchEnded { .. } => return Ok(()),
            ServerToClient::Error { .. } => {
                // The server will re-prompt with another AwaitAction;
                // nothing to do here. A real client may surface the
                // error to the user, but that is TUI concern.
            }
        }
    }
}
