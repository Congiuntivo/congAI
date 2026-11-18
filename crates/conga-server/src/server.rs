//! Authoritative match loop.
//!
//! The server owns the rule set, the match state, one `Transport` per
//! seat, a seeded `RandomChance` and a `MatchLogWriter`. Clients never
//! touch the engine; every action goes through `legal_actions` before
//! `apply`, and every event is broadcast to every seat with a per-seat
//! monotonic sequence number.

use std::fmt;

use conga_core::{Action, ActionList, HandResult, RandomChance, RuleSet, Seat};
use conga_protocol::{ClientToServer, ErrorCode, ServerToClient};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

use crate::log::{MatchLogRecord, MatchLogWriter};
use crate::match_state::MatchState;
use crate::transport::{Transport, TransportError};

/// Errors the server loop can bubble up. Distinct from
/// [`TransportError`] because the server layer adds its own failure
/// modes (log IO, bad peer messages the loop cannot recover from).
#[derive(Debug)]
pub enum ServerError {
    Transport(TransportError),
    Log(std::io::Error),
    /// Reached when a seat we were waiting for closed its transport
    /// mid-hand with no `MatchEnded` sent. m2 treats this as fatal; a
    /// later milestone adds reconnect / AI take-over via
    /// `TimeoutPolicy`.
    SeatDisconnected(Seat),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::Transport(e) => write!(f, "transport: {e}"),
            ServerError::Log(e) => write!(f, "log io: {e}"),
            ServerError::SeatDisconnected(s) => {
                write!(f, "seat {} disconnected mid-match", s.index())
            }
        }
    }
}

impl std::error::Error for ServerError {}

impl From<TransportError> for ServerError {
    fn from(value: TransportError) -> Self {
        ServerError::Transport(value)
    }
}

impl From<std::io::Error> for ServerError {
    fn from(value: std::io::Error) -> Self {
        ServerError::Log(value)
    }
}

pub struct GameServer<R, T, L>
where
    R: RuleSet,
    T: Transport<Outgoing = ServerToClient, Incoming = ClientToServer>,
    L: MatchLogWriter,
{
    rules: R,
    match_state: MatchState,
    transports: Vec<T>,
    chance: RandomChance<ChaCha8Rng>,
    log: L,
    event_seq_per_seat: Vec<u32>,
    seed: u64,
}

impl<R, T, L> GameServer<R, T, L>
where
    R: RuleSet,
    T: Transport<Outgoing = ServerToClient, Incoming = ClientToServer>,
    L: MatchLogWriter,
{
    pub fn new(
        rules: R,
        seats_count: u8,
        starting_dealer: Seat,
        transports: Vec<T>,
        seed: u64,
        log: L,
    ) -> Self {
        assert_eq!(
            transports.len(),
            seats_count as usize,
            "need exactly one transport per seat"
        );
        let rng = ChaCha8Rng::seed_from_u64(seed);
        GameServer {
            rules,
            match_state: MatchState::new(seats_count, starting_dealer),
            transports,
            chance: RandomChance::new(rng),
            log,
            event_seq_per_seat: vec![0; seats_count as usize],
            seed,
        }
    }

    pub fn match_state(&self) -> &MatchState {
        &self.match_state
    }

    /// Drive the match until it ends. Emits a `MatchHeader` first, then
    /// one `HandHeader` + shuffles + actions + `HandResult` per hand,
    /// then one `MatchEnd` record.
    pub async fn run(&mut self) -> Result<(), ServerError> {
        let ruleset_id = self.rules.id();
        self.log.write_record(&MatchLogRecord::match_header(
            ruleset_id,
            self.match_state.seats_count,
            self.match_state.dealer,
            self.match_state.scores.clone(),
            self.seed,
        ))?;

        while !self.match_state.is_finished() {
            let hand_index = self.match_state.hands_played;
            let result = self.run_one_hand(hand_index).await?;
            self.log.write_record(&MatchLogRecord::HandResult {
                result: result.clone(),
            })?;
            for transport in &mut self.transports {
                transport
                    .send(ServerToClient::HandEnded {
                        result: result.clone(),
                    })
                    .await?;
            }
            self.match_state.finish_hand(&self.rules, &result);
        }

        self.log.write_record(&MatchLogRecord::MatchEnd {
            status: self.match_state.status,
            final_scores: self.match_state.scores.clone(),
        })?;
        for transport in &mut self.transports {
            transport
                .send(ServerToClient::MatchEnded {
                    status: self.match_state.status,
                    final_scores: self.match_state.scores.clone(),
                })
                .await?;
        }
        self.log.flush()?;
        Ok(())
    }

    async fn run_one_hand(&mut self, hand_index: u32) -> Result<HandResult, ServerError> {
        let dealer = self.match_state.dealer;
        let starting_scores = self.match_state.scores.clone();

        self.log.write_record(&MatchLogRecord::HandHeader {
            index: hand_index,
            dealer,
            starting_scores: starting_scores.clone(),
        })?;

        let shuffles_before = self.chance.shuffles.len();
        let mut state = self
            .rules
            .new_hand(&starting_scores, dealer, &mut self.chance);
        let mut shuffles_so_far = self.flush_shuffles(shuffles_before)?;

        let mut turn: u32 = 0;
        loop {
            if let Some(result) = self.rules.hand_result(&state) {
                return Ok(result);
            }

            let mut legal = ActionList::new();
            self.rules.legal_actions(&state, &mut legal);

            // Build one observation per seat, broadcast each, then prompt
            // the current seat.
            let seats_count = self.match_state.seats_count;
            let mut observations = Vec::with_capacity(seats_count as usize);
            for s in 0..seats_count {
                observations.push(self.rules.observe(&state, Seat::new(s)));
            }
            let current_seat = observations[0].current_seat;
            for (idx, obs) in observations.into_iter().enumerate() {
                self.transports[idx]
                    .send(ServerToClient::Observe {
                        turn,
                        observation: obs,
                    })
                    .await?;
            }
            self.send_await_action(current_seat, &legal).await?;

            let action = self.recv_action(current_seat, turn, &legal).await?;

            self.log.write_record(&MatchLogRecord::Action {
                seat: current_seat,
                turn,
                action: action.clone(),
            })?;

            let mut events = Vec::new();
            self.rules
                .apply(&mut state, action, &mut self.chance, &mut events);

            shuffles_so_far = self.flush_shuffles(shuffles_so_far)?;

            // Broadcast every event to every seat with its own seq.
            for event in events {
                for (idx, transport) in self.transports.iter_mut().enumerate() {
                    let seq = self.event_seq_per_seat[idx];
                    transport
                        .send(ServerToClient::Event {
                            seq,
                            event: event.clone(),
                        })
                        .await?;
                    self.event_seq_per_seat[idx] = seq + 1;
                }
            }

            turn = turn.saturating_add(1);
        }
    }

    /// Write any shuffle records recorded since `since` to the log, and
    /// return the new "seen" count.
    fn flush_shuffles(&mut self, since: usize) -> Result<usize, ServerError> {
        let now = self.chance.shuffles.len();
        for shuffle in &self.chance.shuffles[since..now] {
            self.log.write_record(&MatchLogRecord::Shuffle {
                cards: shuffle.clone(),
            })?;
        }
        Ok(now)
    }

    async fn send_await_action(&mut self, seat: Seat, legal: &[Action]) -> Result<(), ServerError> {
        self.transports[seat.index() as usize]
            .send(ServerToClient::AwaitAction {
                legal: legal.to_vec(),
                turn_timer_ms: None,
                timeout_policy: None,
            })
            .await?;
        Ok(())
    }

    /// Wait for the acting seat's `Act`. Rejects stale turn numbers with
    /// `WrongTurn` and illegal actions with `IllegalAction`, re-prompting
    /// in both cases. Ignores a `Resume` arriving outside the m9 reconnect
    /// flow.
    async fn recv_action(
        &mut self,
        seat: Seat,
        expected_turn: u32,
        legal: &[Action],
    ) -> Result<Action, ServerError> {
        loop {
            let msg = self.transports[seat.index() as usize].recv().await;
            match msg {
                Err(TransportError::ChannelClosed) => {
                    return Err(ServerError::SeatDisconnected(seat));
                }
                Err(other) => return Err(ServerError::Transport(other)),
                Ok(ClientToServer::Act { turn, action }) => {
                    if turn != expected_turn {
                        self.transports[seat.index() as usize]
                            .send(ServerToClient::Error {
                                code: ErrorCode::WrongTurn,
                                message: format!("expected turn {expected_turn}, got {turn}"),
                            })
                            .await?;
                        self.send_await_action(seat, legal).await?;
                        continue;
                    }
                    if !legal.contains(&action) {
                        self.transports[seat.index() as usize]
                            .send(ServerToClient::Error {
                                code: ErrorCode::IllegalAction,
                                message: format!("action not in legal_actions: {action:?}"),
                            })
                            .await?;
                        self.send_await_action(seat, legal).await?;
                        continue;
                    }
                    return Ok(action);
                }
                Ok(ClientToServer::Resume { .. }) => {
                    // Reserved for m9 reconnect. Silently ignore; the
                    // seat will try again.
                    continue;
                }
            }
        }
    }
}
