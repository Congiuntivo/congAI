//! End-to-end round-trip tests: build an envelope, write it through
//! `write_message`, read it back through `read_message`, assert equality.
//!
//! Also exercises the error paths: protocol-version mismatch, malformed
//! JSON and clean EOF.

use std::io::{BufReader, Cursor};

use conga_core::{
    Action, Bonus, Card, DrawSource, Event, Hand, HandResult, MatchStatus, Meld, MeldKind,
    Observation, Phase, Rank, Seat, Suit,
};
use conga_protocol::{
    ClientToServer, Envelope, ErrorCode, PROTOCOL_VERSION, ProtocolError, ServerToClient,
    TimeoutPolicy, read_message, write_message,
};

const RULESET_HASH: u64 = 0xCAFE_F00D;

fn c(suit: Suit, rank: u8) -> Card {
    Card::new(suit, Rank::new(rank).unwrap())
}

fn roundtrip_client(msg: ClientToServer) {
    let env = Envelope::new(msg.clone(), RULESET_HASH);
    let mut buf = Vec::new();
    write_message(&mut buf, &env).expect("write");
    let mut reader = BufReader::new(Cursor::new(buf));
    let back: Envelope<ClientToServer> = read_message(&mut reader).expect("read");
    assert_eq!(back.protocol_version, PROTOCOL_VERSION);
    assert_eq!(back.ruleset_hash, RULESET_HASH);
    assert_eq!(back.body, msg);
}

fn roundtrip_server(msg: ServerToClient) {
    let env = Envelope::new(msg.clone(), RULESET_HASH);
    let mut buf = Vec::new();
    write_message(&mut buf, &env).expect("write");
    let mut reader = BufReader::new(Cursor::new(buf));
    let back: Envelope<ServerToClient> = read_message(&mut reader).expect("read");
    assert_eq!(back.body, msg);
}

// --- ClientToServer ---

#[test]
fn client_act_round_trips() {
    roundtrip_client(ClientToServer::Act {
        turn: 5,
        action: Action::DrawStock,
    });
    roundtrip_client(ClientToServer::Act {
        turn: 12,
        action: Action::Discard(c(Suit::Clubs, 7)),
    });
    roundtrip_client(ClientToServer::Act {
        turn: 20,
        action: Action::Close(c(Suit::Hearts, 2)),
    });
    roundtrip_client(ClientToServer::Act {
        turn: 30,
        action: Action::LayMeld(Meld {
            kind: MeldKind::Set,
            cards: Hand::from(
                [c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)].as_slice(),
            ),
        }),
    });
    roundtrip_client(ClientToServer::Act {
        turn: 31,
        action: Action::AddToMeld {
            card: c(Suit::Spades, 7),
            meld_index: 0,
        },
    });
    roundtrip_client(ClientToServer::Act {
        turn: 32,
        action: Action::Pass,
    });
}

#[test]
fn client_resume_round_trips() {
    roundtrip_client(ClientToServer::Resume { since_event_seq: 0 });
    roundtrip_client(ClientToServer::Resume {
        since_event_seq: 1234,
    });
}

// --- ServerToClient ---

fn sample_observation() -> Observation {
    Observation {
        own_seat: Seat::new(0),
        current_seat: Seat::new(0),
        phase: Phase::Discard,
        own_hand: Hand::from([c(Suit::Clubs, 7), c(Suit::Hearts, 2)].as_slice()),
        top_discard: Some(c(Suit::Spades, 5)),
        discard_history: vec![c(Suit::Spades, 5)],
        stock_size: 24,
        melds_on_table: Vec::new(),
        scores: vec![0, 0],
        hand_sizes: vec![2, 7],
    }
}

#[test]
fn server_observe_round_trips() {
    roundtrip_server(ServerToClient::Observe {
        turn: 3,
        observation: sample_observation(),
    });
}

#[test]
fn server_await_action_round_trips() {
    roundtrip_server(ServerToClient::AwaitAction {
        legal: vec![Action::DrawStock, Action::DrawDiscard],
        turn_timer_ms: None,
        timeout_policy: None,
    });
    // With the reserved fields populated, to prove serde reads them both.
    roundtrip_server(ServerToClient::AwaitAction {
        legal: vec![Action::Pass],
        turn_timer_ms: Some(30_000),
        timeout_policy: Some(TimeoutPolicy::TakeOverByAi),
    });
}

#[test]
fn server_event_round_trips() {
    roundtrip_server(ServerToClient::Event {
        seq: 0,
        event: Event::Dealt {
            seat: Seat::new(0),
            hand_size: 7,
        },
    });
    roundtrip_server(ServerToClient::Event {
        seq: 5,
        event: Event::Drew {
            seat: Seat::new(1),
            source: DrawSource::TopDiscard,
        },
    });
}

#[test]
fn server_hand_ended_round_trips() {
    roundtrip_server(ServerToClient::HandEnded {
        result: HandResult {
            points: vec![-10, 8],
            closer: Some(Seat::new(0)),
            bonus: Some(Bonus::MinusTen),
        },
    });
}

#[test]
fn server_match_ended_round_trips() {
    roundtrip_server(ServerToClient::MatchEnded {
        status: MatchStatus::Winner(Seat::new(0)),
        final_scores: vec![42, 105],
    });
    roundtrip_server(ServerToClient::MatchEnded {
        status: MatchStatus::InProgress,
        final_scores: vec![0, 0],
    });
}

#[test]
fn server_error_round_trips() {
    for code in [
        ErrorCode::IllegalAction,
        ErrorCode::WrongTurn,
        ErrorCode::ProtocolMismatch,
        ErrorCode::MalformedMessage,
        ErrorCode::InternalError,
    ] {
        roundtrip_server(ServerToClient::Error {
            code,
            message: "nope".into(),
        });
    }
}

// --- Wire-level shape and error paths ---

#[test]
fn wire_is_exactly_one_line_per_message() {
    let env = Envelope::new(
        ClientToServer::Act {
            turn: 7,
            action: Action::DrawStock,
        },
        RULESET_HASH,
    );
    let mut buf = Vec::new();
    write_message(&mut buf, &env).unwrap();
    let s = std::str::from_utf8(&buf).unwrap();
    assert!(s.ends_with('\n'), "line not newline-terminated: {s:?}");
    assert_eq!(s.matches('\n').count(), 1, "expected exactly one newline");
}

#[test]
fn two_messages_read_sequentially() {
    let a = Envelope::new(
        ClientToServer::Act {
            turn: 1,
            action: Action::DrawStock,
        },
        RULESET_HASH,
    );
    let b = Envelope::new(
        ClientToServer::Act {
            turn: 2,
            action: Action::Pass,
        },
        RULESET_HASH,
    );
    let mut buf = Vec::new();
    write_message(&mut buf, &a).unwrap();
    write_message(&mut buf, &b).unwrap();
    let mut reader = BufReader::new(Cursor::new(buf));
    let read_a: Envelope<ClientToServer> = read_message(&mut reader).unwrap();
    let read_b: Envelope<ClientToServer> = read_message(&mut reader).unwrap();
    assert_eq!(read_a.body, a.body);
    assert_eq!(read_b.body, b.body);
}

#[test]
fn protocol_version_mismatch_is_detected() {
    // Hand-crafted line with protocol_version=999.
    let line =
        br#"{"protocol_version":999,"ruleset_hash":0,"body":{"type":"Resume","since_event_seq":0}}
"#;
    let mut reader = BufReader::new(Cursor::new(&line[..]));
    let err = read_message::<_, ClientToServer>(&mut reader).unwrap_err();
    match err {
        ProtocolError::ProtocolMismatch { got, want } => {
            assert_eq!(got, 999);
            assert_eq!(want, PROTOCOL_VERSION);
        }
        other => panic!("expected ProtocolMismatch, got {other:?}"),
    }
}

#[test]
fn malformed_json_returns_parse_error_not_panic() {
    let line = b"not json at all\n";
    let mut reader = BufReader::new(Cursor::new(&line[..]));
    let err = read_message::<_, ClientToServer>(&mut reader).unwrap_err();
    assert!(
        matches!(err, ProtocolError::Parse(_)),
        "expected ParseError, got {err:?}"
    );
}

#[test]
fn clean_eof_returns_unexpected_eof() {
    let mut reader = BufReader::new(Cursor::new(Vec::<u8>::new()));
    let err = read_message::<_, ClientToServer>(&mut reader).unwrap_err();
    assert!(
        matches!(err, ProtocolError::UnexpectedEof),
        "expected UnexpectedEof, got {err:?}"
    );
}

#[test]
fn envelope_ruleset_hash_round_trips() {
    let env = Envelope {
        protocol_version: PROTOCOL_VERSION,
        ruleset_hash: 0xDEAD_BEEF_CAFE_BABE,
        body: ClientToServer::Resume { since_event_seq: 1 },
    };
    let mut buf = Vec::new();
    write_message(&mut buf, &env).unwrap();
    let mut reader = BufReader::new(Cursor::new(buf));
    let back: Envelope<ClientToServer> = read_message(&mut reader).unwrap();
    assert_eq!(back.ruleset_hash, 0xDEAD_BEEF_CAFE_BABE);
}

#[test]
fn envelope_body_shape_uses_tag_field() {
    // Verifies the on-wire shape: the client-to-server body carries a
    // "type" discriminant alongside its fields, no nesting.
    let env = Envelope::new(
        ClientToServer::Act {
            turn: 7,
            action: Action::DrawStock,
        },
        RULESET_HASH,
    );
    let json = serde_json::to_string(&env).unwrap();
    assert!(
        json.contains("\"body\":{\"type\":\"Act\""),
        "body shape not as expected: {json}"
    );
}
