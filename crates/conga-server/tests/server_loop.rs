//! Integration tests for the server loop.
//!
//! Each test wires one `GameServer` to two in-memory transports, drives
//! the clients from async tasks and asserts on outcomes (scores, log
//! records, error paths).

use std::sync::{Arc, Mutex};

use conga_core::{Action, MatchStatus, Seat};
use conga_protocol::{ClientToServer, ErrorCode, ServerToClient};
use conga_rules::StandardConga;
use conga_server::{
    ClientEnd, GameServer, MatchLogRecord, MatchLogWriter, Transport, in_memory_pair,
};
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

/// A `MatchLogWriter` that keeps every record in a `Vec` so tests can
/// inspect what the server logged.
#[derive(Clone, Default)]
struct VecLog {
    records: Arc<Mutex<Vec<MatchLogRecord>>>,
}

impl VecLog {
    fn records(&self) -> Vec<MatchLogRecord> {
        self.records.lock().unwrap().clone()
    }
}

impl MatchLogWriter for VecLog {
    fn write_record(&mut self, rec: &MatchLogRecord) -> std::io::Result<()> {
        self.records.lock().unwrap().push(rec.clone());
        Ok(())
    }
}

/// Minimal random client: reads messages, remembers the latest turn and
/// legal actions, replies with a uniformly-chosen legal action, exits
/// on `MatchEnded`.
async fn run_random_client(mut end: ClientEnd, seed: u64) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut latest_turn: u32 = 0;
    loop {
        let msg = match end.recv().await {
            Ok(m) => m,
            Err(_) => return,
        };
        match msg {
            ServerToClient::Observe { turn, .. } => latest_turn = turn,
            ServerToClient::AwaitAction { legal, .. } => {
                let idx = (rng.next_u32() as usize) % legal.len();
                let action = legal[idx].clone();
                if end
                    .send(ClientToServer::Act {
                        turn: latest_turn,
                        action,
                    })
                    .await
                    .is_err()
                {
                    return;
                }
            }
            ServerToClient::MatchEnded { .. } => return,
            _ => {}
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn random_vs_random_match_terminates_with_a_winner() {
    let (s0, c0) = in_memory_pair();
    let (s1, c1) = in_memory_pair();
    let log = VecLog::default();

    let mut server = GameServer::new(
        StandardConga::default(),
        2,
        Seat::new(1),
        vec![s0, s1],
        0xC0FFEE,
        log.clone(),
    );

    let client_a = tokio::spawn(run_random_client(c0, 0xA1A1_A1A1));
    let client_b = tokio::spawn(run_random_client(c1, 0xB2B2_B2B2));

    server.run().await.expect("server run");
    client_a.await.unwrap();
    client_b.await.unwrap();

    let final_state = server.match_state();
    match final_state.status {
        MatchStatus::Winner(_) => {}
        other => panic!("expected a Winner, got {other:?}"),
    }
    assert!(final_state.hands_played >= 1);

    // At least one score exceeds 100 OR all remaining scores are equal
    // (rule 16 tie-break) — some match path must have produced a winner.
    let records = log.records();
    assert!(
        matches!(records.first(), Some(MatchLogRecord::MatchHeader { .. })),
        "first log record must be MatchHeader"
    );
    assert!(
        matches!(records.last(), Some(MatchLogRecord::MatchEnd { .. })),
        "last log record must be MatchEnd"
    );
    // Dealer rotated once per hand (+1 mod 2 => ping-pongs). After n hands,
    // dealer == starting_dealer + n mod 2.
    let hands = final_state.hands_played;
    let expected_dealer = (1 + hands as u8) % 2;
    assert_eq!(final_state.dealer, Seat::new(expected_dealer));
}

/// A scripted-first-action client: the client whose first `AwaitAction`
/// gets the forbidden action [`send_first`] sends it, then reverts to a
/// random strategy. Lets us verify the server rejects an illegal action.
async fn run_illegal_then_random_client(
    mut end: ClientEnd,
    seed: u64,
    send_first: Action,
    out: Arc<Mutex<Vec<ServerToClient>>>,
) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut latest_turn: u32 = 0;
    let mut did_first_illegal = false;
    loop {
        let msg = match end.recv().await {
            Ok(m) => m,
            Err(_) => return,
        };
        out.lock().unwrap().push(msg.clone());
        match msg {
            ServerToClient::Observe { turn, .. } => latest_turn = turn,
            ServerToClient::AwaitAction { legal, .. } => {
                let action = if !did_first_illegal {
                    did_first_illegal = true;
                    send_first.clone()
                } else {
                    let idx = (rng.next_u32() as usize) % legal.len();
                    legal[idx].clone()
                };
                if end
                    .send(ClientToServer::Act {
                        turn: latest_turn,
                        action,
                    })
                    .await
                    .is_err()
                {
                    return;
                }
            }
            ServerToClient::MatchEnded { .. } => return,
            _ => {}
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn illegal_action_is_rejected_and_server_re_prompts() {
    let (s0, c0) = in_memory_pair();
    let (s1, c1) = in_memory_pair();

    // With dealer = seat 1, seat 0 starts in the Discard phase (C3).
    // Sending DrawStock on the first turn is illegal (wrong phase).
    let seen_by_seat_0 = Arc::new(Mutex::new(Vec::new()));

    let mut server = GameServer::new(
        StandardConga::default(),
        2,
        Seat::new(1),
        vec![s0, s1],
        0xDEAD_BEEF,
        conga_server::NullWriter,
    );

    let client_a = tokio::spawn(run_illegal_then_random_client(
        c0,
        0xA0A0,
        Action::DrawStock,
        seen_by_seat_0.clone(),
    ));
    let client_b = tokio::spawn(run_random_client(c1, 0xB0B0));

    server.run().await.expect("server run");
    client_a.await.unwrap();
    client_b.await.unwrap();

    let msgs = seen_by_seat_0.lock().unwrap().clone();
    let any_illegal_error = msgs.iter().any(|m| {
        matches!(
            m,
            ServerToClient::Error {
                code: ErrorCode::IllegalAction,
                ..
            }
        )
    });
    assert!(
        any_illegal_error,
        "expected the server to send Error {{ IllegalAction }} to seat 0"
    );

    // After the error the server must have re-prompted, so the final
    // state still reaches MatchEnded.
    assert!(
        msgs.iter()
            .any(|m| matches!(m, ServerToClient::MatchEnded { .. }))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn match_header_and_hand_header_carry_correct_bookkeeping() {
    let (s0, c0) = in_memory_pair();
    let (s1, c1) = in_memory_pair();
    let log = VecLog::default();

    let mut server = GameServer::new(
        StandardConga::default(),
        2,
        Seat::new(1),
        vec![s0, s1],
        0x12345,
        log.clone(),
    );
    let client_a = tokio::spawn(run_random_client(c0, 11));
    let client_b = tokio::spawn(run_random_client(c1, 22));
    server.run().await.unwrap();
    client_a.await.unwrap();
    client_b.await.unwrap();

    let records = log.records();
    let header = records
        .iter()
        .find(|r| matches!(r, MatchLogRecord::MatchHeader { .. }))
        .expect("has match header");
    match header {
        MatchLogRecord::MatchHeader {
            protocol_version,
            seats,
            starting_dealer,
            starting_scores,
            seed,
            ..
        } => {
            assert_eq!(*protocol_version, conga_protocol::PROTOCOL_VERSION);
            assert_eq!(*seats, 2);
            assert_eq!(*starting_dealer, Seat::new(1));
            assert_eq!(starting_scores, &vec![0, 0]);
            assert_eq!(*seed, 0x12345);
        }
        _ => unreachable!(),
    }

    // The first hand_header must have index 0.
    let first_hand_header = records
        .iter()
        .find(|r| matches!(r, MatchLogRecord::HandHeader { .. }))
        .expect("has hand header");
    match first_hand_header {
        MatchLogRecord::HandHeader {
            index,
            dealer,
            starting_scores,
        } => {
            assert_eq!(*index, 0);
            assert_eq!(*dealer, Seat::new(1));
            assert_eq!(starting_scores, &vec![0, 0]);
        }
        _ => unreachable!(),
    }
}
