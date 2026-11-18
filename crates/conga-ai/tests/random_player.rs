//! `RandomPlayer` and `run_player` driver tests.

use conga_ai::{Player, RandomPlayer, run_player};
use conga_core::{
    Action, Bonus, Card, Hand, HandResult, MatchStatus, Observation, Phase, Rank, Seat, Suit,
};
use conga_protocol::{ClientToServer, ServerToClient};
use conga_server::{Transport, in_memory_pair};

fn c(suit: Suit, rank: u8) -> Card {
    Card::new(suit, Rank::new(rank).unwrap())
}

fn sample_obs() -> Observation {
    Observation {
        own_seat: Seat::new(0),
        current_seat: Seat::new(0),
        phase: Phase::Draw,
        own_hand: Hand::from([c(Suit::Clubs, 7), c(Suit::Hearts, 2)].as_slice()),
        top_discard: Some(c(Suit::Spades, 5)),
        discard_history: vec![c(Suit::Spades, 5)],
        stock_size: 25,
        melds_on_table: Vec::new(),
        scores: vec![0, 0],
        hand_sizes: vec![2, 7],
    }
}

#[test]
fn random_player_returns_a_legal_action() {
    let mut player = RandomPlayer::from_seed(42);
    let legal = vec![
        Action::DrawStock,
        Action::DrawDiscard,
        Action::Pass,
        Action::Discard(c(Suit::Clubs, 7)),
    ];
    let obs = sample_obs();
    for _ in 0..500 {
        let chosen = player.act(&obs, &legal);
        assert!(
            legal.contains(&chosen),
            "RandomPlayer returned a non-legal action: {chosen:?}"
        );
    }
}

#[test]
fn random_player_distribution_is_close_to_uniform() {
    // 10,000 draws over a 4-action list; 3σ around the mean of 2,500
    // is about 1,500..3,500. Comfortably wider than any realistic bias
    // and still tight enough to catch a broken distribution.
    let mut player = RandomPlayer::from_seed(0xBEEF);
    let legal = vec![
        Action::DrawStock,
        Action::DrawDiscard,
        Action::Pass,
        Action::Discard(c(Suit::Clubs, 7)),
    ];
    let obs = sample_obs();
    let mut counts = [0u32; 4];
    for _ in 0..10_000 {
        let chosen = player.act(&obs, &legal);
        let idx = legal.iter().position(|a| a == &chosen).unwrap();
        counts[idx] += 1;
    }
    for (i, &count) in counts.iter().enumerate() {
        assert!(
            (1_500..=3_500).contains(&count),
            "bucket {i} out of 3σ band: {count}"
        );
    }
}

#[test]
fn random_player_same_seed_is_deterministic() {
    let obs = sample_obs();
    let legal = vec![
        Action::DrawStock,
        Action::DrawDiscard,
        Action::Pass,
        Action::Discard(c(Suit::Clubs, 7)),
    ];

    let mut p1 = RandomPlayer::from_seed(123);
    let mut p2 = RandomPlayer::from_seed(123);
    for _ in 0..100 {
        assert_eq!(p1.act(&obs, &legal), p2.act(&obs, &legal));
    }
}

#[test]
#[should_panic(expected = "no legal actions")]
fn random_player_panics_on_empty_legal() {
    let mut player = RandomPlayer::from_seed(1);
    let obs = sample_obs();
    let _ = player.act(&obs, &[]);
}

#[tokio::test(flavor = "current_thread")]
async fn driver_sends_one_act_and_exits_on_match_ended() {
    // The mock "server" sends: Observe, AwaitAction, MatchEnded.
    // It then waits for exactly one Act from the player.
    let (mut server, client) = in_memory_pair();

    let server_task = tokio::spawn(async move {
        server
            .send(ServerToClient::Observe {
                turn: 0,
                observation: sample_obs(),
            })
            .await
            .unwrap();
        server
            .send(ServerToClient::AwaitAction {
                legal: vec![Action::DrawStock, Action::DrawDiscard],
                turn_timer_ms: None,
                timeout_policy: None,
            })
            .await
            .unwrap();
        // Read the Act before we close the match down.
        let act = server.recv().await.unwrap();
        server
            .send(ServerToClient::MatchEnded {
                status: MatchStatus::Winner(Seat::new(0)),
                final_scores: vec![0, 101],
            })
            .await
            .unwrap();
        act
    });

    let player = RandomPlayer::from_seed(7);
    run_player(player, client).await.expect("driver exit");
    let act = server_task.await.unwrap();
    match act {
        ClientToServer::Act { turn, action } => {
            assert_eq!(turn, 0);
            assert!(matches!(action, Action::DrawStock | Action::DrawDiscard));
        }
        other => panic!("expected Act, got {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn driver_delivers_events_to_notify() {
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct Spy {
        events: Arc<Mutex<Vec<conga_core::Event>>>,
    }

    impl Player for Spy {
        fn act(&mut self, _obs: &Observation, legal: &[Action]) -> Action {
            legal[0].clone()
        }
        fn notify(&mut self, event: &conga_core::Event) {
            self.events.lock().unwrap().push(event.clone());
        }
    }

    let events = Arc::new(Mutex::new(Vec::new()));
    let spy = Spy {
        events: events.clone(),
    };

    let (mut server, client) = in_memory_pair();
    let server_task = tokio::spawn(async move {
        server
            .send(ServerToClient::Observe {
                turn: 0,
                observation: sample_obs(),
            })
            .await
            .unwrap();
        server
            .send(ServerToClient::Event {
                seq: 0,
                event: conga_core::Event::Passed { seat: Seat::new(1) },
            })
            .await
            .unwrap();
        server
            .send(ServerToClient::Event {
                seq: 1,
                event: conga_core::Event::HandEnded(HandResult {
                    points: vec![0, 10],
                    closer: None,
                    bonus: None,
                }),
            })
            .await
            .unwrap();
        server
            .send(ServerToClient::MatchEnded {
                status: MatchStatus::Winner(Seat::new(0)),
                final_scores: vec![0, 110],
            })
            .await
            .unwrap();
    });

    run_player(spy, client).await.expect("driver exit");
    server_task.await.unwrap();

    let recorded = events.lock().unwrap();
    assert_eq!(recorded.len(), 2);
    assert!(matches!(recorded[0], conga_core::Event::Passed { .. }));
    assert!(matches!(recorded[1], conga_core::Event::HandEnded(_)));

    // `Bonus` is only used so the import does not go unused.
    let _ = Bonus::MinusTen;
}

#[tokio::test(flavor = "current_thread")]
async fn driver_recovers_from_server_error_message() {
    // The server first sends an Error (which the driver should swallow),
    // then a fresh AwaitAction after an Observe, then MatchEnded.
    let (mut server, client) = in_memory_pair();
    let server_task = tokio::spawn(async move {
        server
            .send(ServerToClient::Observe {
                turn: 0,
                observation: sample_obs(),
            })
            .await
            .unwrap();
        server
            .send(ServerToClient::Error {
                code: conga_protocol::ErrorCode::IllegalAction,
                message: "nope".into(),
            })
            .await
            .unwrap();
        server
            .send(ServerToClient::AwaitAction {
                legal: vec![Action::Pass],
                turn_timer_ms: None,
                timeout_policy: None,
            })
            .await
            .unwrap();
        let act = server.recv().await.unwrap();
        server
            .send(ServerToClient::MatchEnded {
                status: MatchStatus::Winner(Seat::new(0)),
                final_scores: vec![0, 101],
            })
            .await
            .unwrap();
        act
    });
    let player = RandomPlayer::from_seed(3);
    run_player(player, client).await.expect("driver exit");
    let got = server_task.await.unwrap();
    assert!(matches!(
        got,
        ClientToServer::Act {
            action: Action::Pass,
            ..
        }
    ));
}
