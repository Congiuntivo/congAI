//! End-to-end: play a match headlessly (two random players, no TUI),
//! write a JSON-lines log, feed it back through `replay`, assert that
//! every `HandResult` in the log matches the replayed one and that the
//! final `MatchStatus` matches.
//!
//! This is the slice-7 exit-test guarantee.

use std::fs;
use std::io::Cursor;

use conga_cli::{play, replay};
use conga_core::{MatchStatus, Seat};
use conga_rules::StandardConga;
use conga_tui::SuitTheme;

fn temp_log_path(suffix: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let unique = format!(
        "conga-cli-{}-{}-{}.jsonl",
        std::process::id(),
        suffix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    p.push(unique);
    p
}

#[test]
fn match_log_round_trips_through_replay() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let log_path = temp_log_path("roundtrip");
    let (live_status, live_scores) = rt
        .block_on(play::run_two_random_match(
            StandardConga::default(),
            2,
            Seat::new(1),
            0xC0FFEE,
            &log_path,
        ))
        .expect("match ran");
    assert!(matches!(live_status, MatchStatus::Winner(_)));

    // Replay produces the same final status.
    let opts = replay::ReplayOptions {
        log_path: log_path.clone(),
    };
    let mut out = Vec::new();
    let replayed_status = replay::run(opts, Cursor::new(&mut out)).expect("replay ok");
    assert_eq!(replayed_status, live_status);

    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("Match log"));
    assert!(text.contains("=== Hand 0"));
    assert!(text.contains("=== Match end"));
    // Final scores must appear verbatim in the output.
    let scores_debug = format!("{live_scores:?}");
    assert!(
        text.contains(&scores_debug),
        "final scores {scores_debug} missing from replay output"
    );

    let _ = fs::remove_file(log_path);
}

#[test]
fn replay_detects_hand_result_mismatch() {
    // Run a real match, then tamper with the first hand's HandResult
    // record and feed the tampered record list back through replay.
    use conga_server::MatchLogRecord;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let log_path = temp_log_path("tamper");
    let _ = rt
        .block_on(play::run_two_random_match(
            StandardConga::default(),
            2,
            Seat::new(1),
            0xFEED,
            &log_path,
        ))
        .expect("match ran");

    // Load the records and tamper with the first HandResult: add 1 to
    // every point. The replayed HandResult will disagree.
    let file = fs::read_to_string(&log_path).unwrap();
    let mut records: Vec<MatchLogRecord> = file
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let first_result_idx = records
        .iter()
        .position(|r| matches!(r, MatchLogRecord::HandResult { .. }))
        .expect("has a HandResult");
    if let MatchLogRecord::HandResult { result } = &mut records[first_result_idx] {
        for p in &mut result.points {
            *p += 1;
        }
    }

    let mut out = Vec::new();
    let err = replay::replay_records(&records, Cursor::new(&mut out))
        .expect_err("expected mismatch error");
    match err {
        replay::ReplayError::HandResultMismatch { hand_index, .. } => {
            assert_eq!(hand_index, 0);
        }
        other => panic!("expected HandResultMismatch, got {other:?}"),
    }
    let _ = fs::remove_file(log_path);
}

#[test]
fn replay_errors_on_log_without_match_header() {
    use conga_server::MatchLogRecord;
    let records = vec![MatchLogRecord::MatchEnd {
        status: MatchStatus::Winner(Seat::new(0)),
        final_scores: vec![0, 101],
    }];
    let mut out = Vec::new();
    let err = replay::replay_records(&records, Cursor::new(&mut out))
        .expect_err("expected missing header error");
    let msg = format!("{err}");
    assert!(msg.contains("MatchHeader"), "unexpected error: {msg}");
}

#[test]
fn replay_errors_on_log_without_match_end() {
    use conga_server::MatchLogRecord;
    let rules = StandardConga::default();
    use conga_core::RuleSet;
    let ruleset_id = rules.id();
    let records = vec![MatchLogRecord::match_header(
        ruleset_id,
        2,
        Seat::new(1),
        vec![0, 0],
        0,
    )];
    let mut out = Vec::new();
    let err = replay::replay_records(&records, Cursor::new(&mut out))
        .expect_err("expected missing MatchEnd error");
    let msg = format!("{err}");
    assert!(msg.contains("MatchEnd"), "unexpected error: {msg}");
}

#[test]
fn cli_rejects_players_other_than_two() {
    let opts = play::PlayOptions {
        seed: 0,
        players: 3,
        log_path: temp_log_path("badplayers"),
        theme: SuitTheme::French,
        ai: "random".into(),
        samples: 50,
    };
    let err = play::run(opts).expect_err("3 players should fail");
    assert!(matches!(err, play::PlayError::UnsupportedPlayers(3)));
}
