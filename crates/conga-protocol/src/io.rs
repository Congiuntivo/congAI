//! JSON-lines reader and writer.
//!
//! One JSON object per line, UTF-8, `\n`-terminated. The reader returns an
//! `UnexpectedEof` on a clean end-of-stream so a server loop can treat it
//! as a normal disconnect.

use std::io::{BufRead, Write};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::envelope::{Envelope, PROTOCOL_VERSION};
use crate::error::ProtocolError;

/// Read one JSON-lines record and return it as an `Envelope<B>`.
///
/// Returns `UnexpectedEof` on a clean EOF, `Parse` on malformed JSON, and
/// `ProtocolMismatch` when the envelope's `protocol_version` does not
/// match `PROTOCOL_VERSION`.
pub fn read_message<R: BufRead, B: DeserializeOwned>(
    reader: &mut R,
) -> Result<Envelope<B>, ProtocolError> {
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Err(ProtocolError::UnexpectedEof);
    }
    let env: Envelope<B> = serde_json::from_str(line.trim_end_matches(['\r', '\n']))?;
    if env.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::ProtocolMismatch {
            got: env.protocol_version,
            want: PROTOCOL_VERSION,
        });
    }
    Ok(env)
}

/// Write one JSON-lines record. Flushes nothing; the caller decides when
/// to flush.
pub fn write_message<W: Write, B: Serialize>(
    writer: &mut W,
    envelope: &Envelope<B>,
) -> Result<(), ProtocolError> {
    serde_json::to_writer(&mut *writer, envelope)?;
    writer.write_all(b"\n")?;
    Ok(())
}
