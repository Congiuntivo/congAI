//! JSON-lines match log. The server writes one `MatchLogRecord` per line;
//! the replay CLI (slice 7) reads them back and reconstructs the match.
//!
//! The log contains only facts the server generated: headers, shuffles,
//! applied actions, hand results, and the final match status. Events are
//! not logged — they are a function of (state, action) and the replay
//! path recomputes them.

use std::io::Write;

use conga_core::{Action, Card, HandResult, MatchStatus, RuleSetId, Seat};
use conga_protocol::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MatchLogRecord {
    MatchHeader {
        protocol_version: u32,
        ruleset_id: RuleSetId,
        seats: u8,
        starting_dealer: Seat,
        starting_scores: Vec<i32>,
        seed: u64,
    },
    HandHeader {
        index: u32,
        dealer: Seat,
        starting_scores: Vec<i32>,
    },
    /// One shuffle result, in the order cards were dealt or drawn. Multiple
    /// records may appear per hand due to the rule-14 empty-stock reshuffle.
    Shuffle {
        cards: Vec<Card>,
    },
    Action {
        seat: Seat,
        turn: u32,
        action: Action,
    },
    HandResult {
        result: HandResult,
    },
    MatchEnd {
        status: MatchStatus,
        final_scores: Vec<i32>,
    },
}

impl MatchLogRecord {
    pub fn match_header(
        ruleset_id: RuleSetId,
        seats: u8,
        starting_dealer: Seat,
        starting_scores: Vec<i32>,
        seed: u64,
    ) -> Self {
        MatchLogRecord::MatchHeader {
            protocol_version: PROTOCOL_VERSION,
            ruleset_id,
            seats,
            starting_dealer,
            starting_scores,
            seed,
        }
    }
}

pub trait MatchLogWriter: Send {
    fn write_record(&mut self, rec: &MatchLogRecord) -> std::io::Result<()>;
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct JsonLinesWriter<W: Write> {
    writer: W,
}

impl<W: Write> JsonLinesWriter<W> {
    pub fn new(writer: W) -> Self {
        JsonLinesWriter { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write + Send> MatchLogWriter for JsonLinesWriter<W> {
    fn write_record(&mut self, rec: &MatchLogRecord) -> std::io::Result<()> {
        let json = serde_json::to_string(rec)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.writer.write_all(json.as_bytes())?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// Writer that drops every record. Useful for tests that exercise the
/// server loop but do not inspect the log.
pub struct NullWriter;

impl MatchLogWriter for NullWriter {
    fn write_record(&mut self, _rec: &MatchLogRecord) -> std::io::Result<()> {
        Ok(())
    }
}
