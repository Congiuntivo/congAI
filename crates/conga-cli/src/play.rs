//! `conga-cli play` — run a full 2-player match.
//!
//! Spawns the authoritative server, a TUI-driven seat 0 (the human)
//! and a `RandomPlayer`-driven seat 1 (the AI), all talking over
//! in-memory transports. Writes a JSON-lines match log to `--log`.
//!
//! The TUI variant goes through the real terminal. For tests, use
//! [`run_two_random_match`] which replaces both human and AI with
//! random players so the match runs headlessly.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use conga_ai::{RandomPlayer, run_player};
use conga_core::{MatchStatus, Seat};
use conga_rules::StandardConga;
use conga_server::{GameServer, JsonLinesWriter, MatchLogWriter, in_memory_pair};
use conga_tui::run_tui;

#[derive(Debug)]
pub enum PlayError {
    Io(std::io::Error),
    Server(conga_server::ServerError),
    Tui(conga_tui::TuiError),
    Ai(conga_ai::DriverError),
    Join(tokio::task::JoinError),
    UnsupportedPlayers(u8),
}

impl std::fmt::Display for PlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayError::Io(e) => write!(f, "io: {e}"),
            PlayError::Server(e) => write!(f, "server: {e}"),
            PlayError::Tui(e) => write!(f, "tui: {e}"),
            PlayError::Ai(e) => write!(f, "ai: {e}"),
            PlayError::Join(e) => write!(f, "task join: {e}"),
            PlayError::UnsupportedPlayers(n) => {
                write!(f, "m2 only supports --players 2 (got {n})")
            }
        }
    }
}

impl std::error::Error for PlayError {}

impl From<std::io::Error> for PlayError {
    fn from(value: std::io::Error) -> Self {
        PlayError::Io(value)
    }
}

pub struct PlayOptions {
    pub seed: u64,
    pub players: u8,
    pub log_path: std::path::PathBuf,
}

/// Interactive play: seat 0 is the human via `conga-tui`, seat 1 is a
/// `RandomPlayer`. Runs inside a fresh tokio runtime so `main` can stay
/// sync.
pub fn run(opts: PlayOptions) -> Result<(MatchStatus, Vec<i32>), PlayError> {
    if opts.players != 2 {
        return Err(PlayError::UnsupportedPlayers(opts.players));
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let log_file = File::create(&opts.log_path)?;
        let log = JsonLinesWriter::new(BufWriter::new(log_file));

        let (s0, c0) = in_memory_pair();
        let (s1, c1) = in_memory_pair();

        // Starting dealer = seat 1 so seat 0 plays first — matches the
        // m1 debug CLI convention (the human is seat 0).
        let mut server = GameServer::new(
            StandardConga::default(),
            2,
            Seat::new(1),
            vec![s0, s1],
            opts.seed,
            log,
        );

        let server_task = tokio::spawn(async move {
            server.run().await?;
            let status = server.match_state().status;
            let scores = server.match_state().scores.clone();
            Ok::<_, conga_server::ServerError>((status, scores))
        });

        let tui_task = tokio::spawn(async move { run_tui(c0, 0, 2).await });

        // AI seed derived from the match seed so repeated runs with the
        // same --seed reproduce the AI's actions.
        let ai_seed = opts.seed ^ 0xA1A1_A1A1_A1A1_A1A1;
        let ai_task = tokio::spawn(async move {
            let player = RandomPlayer::from_seed(ai_seed);
            run_player(player, c1).await
        });

        let server_res = server_task.await.map_err(PlayError::Join)?;
        let tui_res = tui_task.await.map_err(PlayError::Join)?;
        let ai_res = ai_task.await.map_err(PlayError::Join)?;

        // Translate each sub-task's error, preferring the server's.
        let (status, scores) = server_res.map_err(PlayError::Server)?;
        // A TUI "user quit" is not a server-side error and is fine to
        // surface as a non-zero exit via the enum.
        tui_res.map_err(PlayError::Tui)?;
        ai_res.map_err(PlayError::Ai)?;

        Ok((status, scores))
    })
}

/// Headless variant used by tests and by the slice-7 replay
/// cross-check: both seats are `RandomPlayer`s. Writes to `log_path`
/// just like interactive play. Returns `(MatchStatus, final_scores)`.
pub async fn run_two_random_match(
    rules: StandardConga,
    seats_count: u8,
    starting_dealer: Seat,
    seed: u64,
    log_path: &Path,
) -> Result<(MatchStatus, Vec<i32>), PlayError> {
    let log_file = File::create(log_path)?;
    let log = JsonLinesWriter::new(BufWriter::new(log_file));
    run_two_random_match_with_log(rules, seats_count, starting_dealer, seed, log).await
}

/// Lower-level variant that takes any `MatchLogWriter`. Used by tests
/// that want to inspect the log records live via a custom writer.
pub async fn run_two_random_match_with_log<L>(
    rules: StandardConga,
    seats_count: u8,
    starting_dealer: Seat,
    seed: u64,
    log: L,
) -> Result<(MatchStatus, Vec<i32>), PlayError>
where
    L: MatchLogWriter + 'static,
{
    assert_eq!(seats_count, 2, "only 2 players supported in m2");

    let (s0, c0) = in_memory_pair();
    let (s1, c1) = in_memory_pair();

    let mut server = GameServer::new(rules, seats_count, starting_dealer, vec![s0, s1], seed, log);

    let server_task = tokio::spawn(async move {
        server.run().await?;
        let status = server.match_state().status;
        let scores = server.match_state().scores.clone();
        Ok::<_, conga_server::ServerError>((status, scores))
    });

    let a_task = tokio::spawn(async move {
        let p = RandomPlayer::from_seed(seed ^ 0xA1A1_A1A1_A1A1_A1A1);
        run_player(p, c0).await
    });
    let b_task = tokio::spawn(async move {
        let p = RandomPlayer::from_seed(seed ^ 0xB2B2_B2B2_B2B2_B2B2);
        run_player(p, c1).await
    });

    let (status, scores) = server_task
        .await
        .map_err(PlayError::Join)?
        .map_err(PlayError::Server)?;
    a_task
        .await
        .map_err(PlayError::Join)?
        .map_err(PlayError::Ai)?;
    b_task
        .await
        .map_err(PlayError::Join)?
        .map_err(PlayError::Ai)?;

    Ok((status, scores))
}
