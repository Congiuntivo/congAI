//! Tests for the in-memory transport pair. Each test is async, so we use
//! `#[tokio::test(flavor = "current_thread")]` to keep the runtime cheap.

use conga_core::{Action, Seat};
use conga_protocol::{ClientToServer, ServerToClient};
use conga_server::{Transport, TransportError, in_memory_pair};

#[tokio::test(flavor = "current_thread")]
async fn messages_round_trip_in_both_directions() {
    let (mut server, mut client) = in_memory_pair();

    // Server -> client.
    server
        .send(ServerToClient::AwaitAction {
            legal: vec![Action::DrawStock, Action::DrawDiscard],
            turn_timer_ms: None,
            timeout_policy: None,
        })
        .await
        .unwrap();
    let got = client.recv().await.unwrap();
    match got {
        ServerToClient::AwaitAction { legal, .. } => {
            assert_eq!(legal, vec![Action::DrawStock, Action::DrawDiscard]);
        }
        other => panic!("unexpected: {other:?}"),
    }

    // Client -> server.
    client
        .send(ClientToServer::Act {
            turn: 1,
            action: Action::DrawStock,
        })
        .await
        .unwrap();
    let got = server.recv().await.unwrap();
    match got {
        ClientToServer::Act { turn, action } => {
            assert_eq!(turn, 1);
            assert_eq!(action, Action::DrawStock);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn messages_preserve_order() {
    let (mut server, mut client) = in_memory_pair();
    for seq in 0..5u32 {
        server
            .send(ServerToClient::Event {
                seq,
                event: conga_core::Event::Passed { seat: Seat::new(0) },
            })
            .await
            .unwrap();
    }
    for seq in 0..5u32 {
        let got = client.recv().await.unwrap();
        match got {
            ServerToClient::Event { seq: got_seq, .. } => assert_eq!(got_seq, seq),
            other => panic!("unexpected: {other:?}"),
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_client_makes_server_recv_return_channel_closed() {
    let (mut server, client) = in_memory_pair();
    drop(client);
    let err = server.recv().await.unwrap_err();
    assert!(matches!(err, TransportError::ChannelClosed));
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_server_makes_client_recv_return_channel_closed() {
    let (server, mut client) = in_memory_pair();
    drop(server);
    let err = client.recv().await.unwrap_err();
    assert!(matches!(err, TransportError::ChannelClosed));
}

#[tokio::test(flavor = "current_thread")]
async fn sending_after_peer_drop_returns_channel_closed() {
    let (mut server, client) = in_memory_pair();
    drop(client);
    let err = server
        .send(ServerToClient::HandEnded {
            result: conga_core::HandResult {
                points: vec![0, 0],
                closer: None,
                bonus: None,
            },
        })
        .await
        .unwrap_err();
    assert!(matches!(err, TransportError::ChannelClosed));
}

#[tokio::test(flavor = "current_thread")]
async fn recv_blocks_until_peer_sends() {
    use tokio::sync::oneshot;
    let (mut server, mut client) = in_memory_pair();
    let (done_tx, done_rx) = oneshot::channel();

    let server_task = tokio::spawn(async move {
        let msg = server.recv().await.unwrap();
        done_tx.send(msg).unwrap();
    });

    // Give the server a chance to park on recv.
    tokio::task::yield_now().await;
    assert!(!server_task.is_finished(), "recv returned before any send");

    client
        .send(ClientToServer::Resume { since_event_seq: 0 })
        .await
        .unwrap();
    let got = done_rx.await.unwrap();
    assert!(matches!(got, ClientToServer::Resume { .. }));
    server_task.await.unwrap();
}
