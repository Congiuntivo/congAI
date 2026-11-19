//! `conga-cli eval` end-to-end: runs the subcommand's `run` fn against
//! real baselines and checks the report shape.

use std::io::Cursor;

use conga_cli::eval::{EvalError, EvalOptions, run};

#[test]
fn eval_prints_a_report_for_random_vs_random() {
    let opts = EvalOptions {
        agent_a: "random".into(),
        agent_b: "random".into(),
        games: 20,
        seed: 1,
        samples: 50,
    };
    let mut out = Vec::new();
    let r = run(opts, Cursor::new(&mut out)).expect("eval ran");
    assert_eq!(r.games, 20);
    assert_eq!(r.a_wins + r.b_wins + r.draws, r.games);

    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Eval: random vs random"));
    assert!(text.contains("95% Wilson CI"));
    assert!(text.contains("Hands per match"));
    assert!(text.contains("Decisions/sec"));
}

#[test]
fn eval_greedy_beats_random_cleanly_on_small_sample() {
    // Not the regression test — just a smoke check that the
    // pipeline produces a greedy-favoured result on a short run.
    let opts = EvalOptions {
        agent_a: "greedy".into(),
        agent_b: "random".into(),
        games: 50,
        seed: 0xC0FFEE,
        samples: 50,
    };
    let mut out = Vec::new();
    let r = run(opts, Cursor::new(&mut out)).expect("eval ran");
    assert!(
        r.a_wins > r.b_wins,
        "greedy should beat random on 50 games (got {} vs {})",
        r.a_wins,
        r.b_wins
    );
}

#[test]
fn eval_rejects_unknown_agent_names() {
    let opts = EvalOptions {
        agent_a: "mcts".into(),
        agent_b: "random".into(),
        games: 2,
        seed: 1,
        samples: 50,
    };
    let err = run(opts, Cursor::new(Vec::new())).expect_err("mcts should be rejected");
    assert!(matches!(err, EvalError::UnknownAgent(ref n) if n == "mcts"));
}

#[test]
fn eval_games_is_rounded_up_to_the_next_even_number_of_matches() {
    // Asking for 3 games means 2 paired matches (4 total matches).
    let opts = EvalOptions {
        agent_a: "random".into(),
        agent_b: "random".into(),
        games: 3,
        seed: 1,
        samples: 50,
    };
    let mut out = Vec::new();
    let r = run(opts, Cursor::new(&mut out)).expect("eval ran");
    assert_eq!(r.games, 4);
}
