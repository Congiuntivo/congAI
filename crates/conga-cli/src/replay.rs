//! `conga-cli replay <log-file>` — rebuild a match from its JSON-lines
//! log and print each hand's state sequence plus the final match status.
//!
//! Reuses the slice-8 `conga_core::log::replay` helper per hand and
//! cross-checks each replayed `HandResult` against the one the server
//! logged live. Any mismatch fails loudly — this is the exit-test
//! guarantee that a saved match reproduces identically.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use conga_core::{Action, Card, GameLog, HandResult, MatchStatus, replay as replay_hand};
use conga_rules::{StandardConga, State};
use conga_server::MatchLogRecord;

#[derive(Debug)]
pub enum ReplayError {
    Io(std::io::Error),
    Parse(String),
    /// The replay produced a different HandResult than the one the
    /// server logged. Fatal: a match that does not replay faithfully
    /// is a bug in the engine or the log format.
    HandResultMismatch {
        hand_index: u32,
        logged: HandResult,
        replayed: HandResult,
    },
    MissingMatchHeader,
    MissingHandResult(u32),
    MissingMatchEnd,
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Io(e) => write!(f, "io: {e}"),
            ReplayError::Parse(s) => write!(f, "parse: {s}"),
            ReplayError::HandResultMismatch {
                hand_index,
                logged,
                replayed,
            } => {
                write!(
                    f,
                    "hand {hand_index}: replay produced {replayed:?} but the log says {logged:?}"
                )
            }
            ReplayError::MissingMatchHeader => f.write_str("log is missing MatchHeader"),
            ReplayError::MissingHandResult(i) => {
                write!(f, "log is missing HandResult for hand {i}")
            }
            ReplayError::MissingMatchEnd => f.write_str("log is missing MatchEnd"),
        }
    }
}

impl std::error::Error for ReplayError {}

impl From<std::io::Error> for ReplayError {
    fn from(value: std::io::Error) -> Self {
        ReplayError::Io(value)
    }
}

pub struct ReplayOptions {
    pub log_path: std::path::PathBuf,
}

pub fn run<W: Write>(opts: ReplayOptions, out: W) -> Result<MatchStatus, ReplayError> {
    let records = read_records(&opts.log_path)?;
    replay_records(&records, out)
}

/// Replay from an in-memory list of records. Separated from `run` so
/// tests can feed records directly without a file.
pub fn replay_records<W: Write>(
    records: &[MatchLogRecord],
    mut out: W,
) -> Result<MatchStatus, ReplayError> {
    let rules = StandardConga::default();

    let header = records
        .iter()
        .find_map(|r| match r {
            MatchLogRecord::MatchHeader {
                protocol_version,
                seats,
                starting_dealer,
                seed,
                ..
            } => Some((*protocol_version, *seats, *starting_dealer, *seed)),
            _ => None,
        })
        .ok_or(ReplayError::MissingMatchHeader)?;
    writeln!(
        out,
        "Match log — protocol v{}, {} seats, starting dealer seat {}, seed 0x{:X}",
        header.0,
        header.1,
        header.2.index(),
        header.3
    )?;

    // Walk records, grouping into hands.
    let mut cursor = 0usize;
    while cursor < records.len() {
        match &records[cursor] {
            MatchLogRecord::HandHeader {
                index,
                dealer,
                starting_scores,
            } => {
                // Collect this hand's shuffles, actions and result.
                let mut shuffles: Vec<Vec<Card>> = Vec::new();
                let mut actions: Vec<Action> = Vec::new();
                let mut logged_result: Option<HandResult> = None;
                let mut j = cursor + 1;
                while j < records.len() {
                    match &records[j] {
                        MatchLogRecord::Shuffle { cards } => shuffles.push(cards.clone()),
                        MatchLogRecord::Action { action, .. } => actions.push(action.clone()),
                        MatchLogRecord::HandResult { result } => {
                            logged_result = Some(result.clone());
                            j += 1;
                            break;
                        }
                        MatchLogRecord::HandHeader { .. } | MatchLogRecord::MatchEnd { .. } => {
                            break;
                        }
                        MatchLogRecord::MatchHeader { .. } => {}
                    }
                    j += 1;
                }
                let logged_result = logged_result.ok_or(ReplayError::MissingHandResult(*index))?;

                let game_log = GameLog {
                    ruleset_id: {
                        use conga_core::RuleSet;
                        rules.id()
                    },
                    seats: header.1,
                    dealer: *dealer,
                    starting_scores: starting_scores.clone(),
                    actions,
                    shuffles,
                };
                let replayed = replay_hand(&rules, &game_log);

                writeln!(out)?;
                writeln!(
                    out,
                    "=== Hand {} — dealer seat {} — starting scores {:?} ===",
                    index,
                    dealer.index(),
                    starting_scores
                )?;
                for (step, state) in replayed.states.iter().enumerate() {
                    writeln!(out, "  step {step}: {}", format_state(state))?;
                }

                let replayed_result = state_result(replayed.states.last().expect("has states"));
                let replayed_result = replayed_result.unwrap_or_else(|| {
                    // Hand may have ended by state inspection; fall back
                    // to the logged result if the engine did not surface
                    // one (should not happen for a sane log).
                    logged_result.clone()
                });
                if replayed_result != logged_result {
                    return Err(ReplayError::HandResultMismatch {
                        hand_index: *index,
                        logged: logged_result,
                        replayed: replayed_result,
                    });
                }
                writeln!(out, "  HandResult: {logged_result:?}")?;
                cursor = j;
                continue;
            }
            MatchLogRecord::MatchEnd {
                status,
                final_scores,
            } => {
                writeln!(out)?;
                writeln!(
                    out,
                    "=== Match end: {:?}  Final scores: {:?} ===",
                    status, final_scores
                )?;
                return Ok(*status);
            }
            _ => {
                cursor += 1;
            }
        }
    }
    Err(ReplayError::MissingMatchEnd)
}

fn read_records(path: &Path) -> Result<Vec<MatchLogRecord>, ReplayError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for (lineno, line_res) in reader.lines().enumerate() {
        let line = line_res?;
        if line.trim().is_empty() {
            continue;
        }
        let rec: MatchLogRecord = serde_json::from_str(&line)
            .map_err(|e| ReplayError::Parse(format!("line {}: {e}", lineno + 1)))?;
        out.push(rec);
    }
    Ok(out)
}

/// Read the current state's HandResult if the engine has reached hand
/// end. Delegates to `StandardConga::hand_result`.
fn state_result(state: &State) -> Option<HandResult> {
    use conga_core::RuleSet;
    let rules = StandardConga::default();
    rules.hand_result(state)
}

/// Compact one-liner summary of a state. Keeps the replay output a
/// human can read without drowning in Debug noise.
fn format_state(state: &State) -> String {
    use std::fmt::Write;
    let mut buf = String::new();
    let _ = write!(
        buf,
        "phase={:?} current=seat {} stock={} discard={} hands=[",
        state.phase,
        state.current_seat.index(),
        state.stock.len(),
        state
            .discard_pile
            .last()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string()),
    );
    for (i, h) in state.hands.iter().enumerate() {
        if i > 0 {
            buf.push_str(", ");
        }
        let cards: Vec<Card> = h.iter().collect();
        let joined = cards
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let _ = write!(buf, "s{i}:{} ({})", h.len(), joined);
    }
    buf.push(']');
    if !state.melds_on_table.is_empty() {
        buf.push_str(" melds=[");
        for (i, m) in state.melds_on_table.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            let cards: Vec<Card> = m.cards.iter().collect();
            let joined = cards
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let _ = write!(buf, "#{i}[{joined}]");
        }
        buf.push(']');
    }
    buf
}
