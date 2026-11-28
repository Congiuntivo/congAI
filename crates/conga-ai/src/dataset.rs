//! Self-play labelling for the value model (D54).
//!
//! `generate(matches_each, seed)` plays paired-deal self-play of the
//! current ladder top (`score-aware` vs `score-aware` and
//! `clean-split-close` vs `clean-split-close`), encodes every
//! `Phase::Draw` or `Phase::Discard` decision via `encoding::encode`
//! and labels it with the committed equity-table lookup at the
//! hand-end scores (seat-relative to the acting player). Showdown
//! decisions are excluded — they aren't the training distribution the
//! rollout-leaf value model will see at search time (D54).
//!
//! The binary dataset format:
//!
//! | Offset | Size | Content                             |
//! | ------ | ---- | ----------------------------------- |
//! | 0      | 8    | magic bytes `CONGADS1`              |
//! | 8      | 4    | `encoding_version` (LE u32)         |
//! | 12     | 4    | `input_size` (LE u32)               |
//! | 16     | 8    | `num_samples` (LE u64)              |
//! | 24     | ...  | `num_samples × (input_size + 1) × f32` little-endian |
//!
//! Each row is `input_size` encoding f32s followed by one label f32.
//! Python reads this with `np.fromfile(..., dtype=np.float32)` after
//! skipping the 24-byte header (slice 5 training script).

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use conga_core::{Action, Event, HandResult, MatchStatus, Phase, RandomChance, RuleSet, Seat};
use conga_rules::StandardConga;
use conga_server::MatchState;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use rayon::prelude::*;

use crate::belief::BeliefTracker;
use crate::encoding::{ENCODING_SIZE, ENCODING_VERSION, encode};
use crate::equity::committed_table;
use crate::eval::BaselineAgent;
use crate::player::Player;

/// Magic bytes that mark a conga-ai dataset file. The trailing `1`
/// tracks the binary layout version independently of
/// `ENCODING_VERSION`; bump on any header change.
pub const MAGIC: &[u8; 8] = b"CONGADS1";

/// Byte size of one row on disk: all encoding floats plus one label.
pub const ROW_BYTES: usize = (ENCODING_SIZE + 1) * 4;

/// Byte size of the fixed header before the rows.
pub const HEADER_BYTES: usize = 8 + 4 + 4 + 8;

/// One labelled training example: an encoded observation plus its
/// hand-end equity label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub encoding: [f32; ENCODING_SIZE],
    pub label: f32,
}

impl Sample {
    /// Phase one-hot indices in the encoding layout (D53). Showdown
    /// samples are never emitted by `generate`; this offset is used by
    /// tests to verify that invariant from the written file.
    pub const PHASE_SHOWDOWN_OFFSET: usize = 250;
}

/// Write `samples` to `path` in the binary format documented at the
/// top of this module.
pub fn write_binary(path: &Path, samples: &[Sample]) -> io::Result<()> {
    let mut f = File::create(path)?;
    f.write_all(MAGIC)?;
    f.write_all(&ENCODING_VERSION.to_le_bytes())?;
    f.write_all(&(ENCODING_SIZE as u32).to_le_bytes())?;
    f.write_all(&(samples.len() as u64).to_le_bytes())?;
    let mut buf = Vec::with_capacity(samples.len() * ROW_BYTES);
    for s in samples {
        for v in &s.encoding {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&s.label.to_le_bytes());
    }
    f.write_all(&buf)?;
    Ok(())
}

/// Read a binary dataset file written by [`write_binary`]. Validates
/// magic, `encoding_version` and `input_size` against the current
/// Rust-side consts; returns `io::ErrorKind::InvalidData` on any
/// mismatch.
pub fn read_binary(path: &Path) -> io::Result<Vec<Sample>> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 8];
    f.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bad magic bytes: {magic:?}"),
        ));
    }
    let mut buf4 = [0u8; 4];
    f.read_exact(&mut buf4)?;
    let encoding_version = u32::from_le_bytes(buf4);
    if encoding_version != ENCODING_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("encoding_version {encoding_version} != current {ENCODING_VERSION}"),
        ));
    }
    f.read_exact(&mut buf4)?;
    let input_size = u32::from_le_bytes(buf4) as usize;
    if input_size != ENCODING_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("input_size {input_size} != current {ENCODING_SIZE}"),
        ));
    }
    let mut buf8 = [0u8; 8];
    f.read_exact(&mut buf8)?;
    let num_samples = u64::from_le_bytes(buf8) as usize;
    let mut out = Vec::with_capacity(num_samples);
    let mut row = vec![0u8; ROW_BYTES];
    for _ in 0..num_samples {
        f.read_exact(&mut row)?;
        let mut encoding = [0.0f32; ENCODING_SIZE];
        for i in 0..ENCODING_SIZE {
            let b = &row[i * 4..i * 4 + 4];
            encoding[i] = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        let lb = &row[ENCODING_SIZE * 4..(ENCODING_SIZE + 1) * 4];
        let label = f32::from_le_bytes([lb[0], lb[1], lb[2], lb[3]]);
        out.push(Sample { encoding, label });
    }
    Ok(out)
}

/// Agent names driving paired self-play. Each (name, name) pair plays
/// against itself; samples are gathered from both seats. Adding a new
/// name here expands the training distribution.
pub const SELF_PLAY_AGENTS: &[&str] = &["score-aware", "clean-split-close"];

/// Play paired-deal self-play of every agent in [`SELF_PLAY_AGENTS`]
/// against itself and return every labelled Draw/Discard decision.
///
/// Rayon-parallel across (agent, match-index) pairs (D43 precedent).
/// Deterministic: the per-match seed is derived from `(seed, agent_idx,
/// match_idx)` so a single call reproduces byte-identical bytes.
pub fn generate(matches_each: u32, seed: u64) -> Vec<Sample> {
    let agents = SELF_PLAY_AGENTS;
    let total = agents.len() as u64 * matches_each as u64;
    (0..total)
        .into_par_iter()
        .flat_map_iter(|k| {
            let agent_idx = (k / matches_each as u64) as usize;
            let match_idx = k % matches_each as u64;
            let per_seed = per_match_seed(seed, agent_idx, match_idx);
            let agent = agents[agent_idx];
            generate_paired_match(agent, agent, per_seed)
        })
        .collect()
}

fn per_match_seed(base: u64, agent_idx: usize, match_idx: u64) -> u64 {
    // Simple, deterministic, independent across (agent, match) pairs.
    base.wrapping_add((agent_idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(match_idx.wrapping_mul(0xBF58_476D_1CE4_E5B9))
}

fn generate_paired_match(a_name: &str, b_name: &str, seed: u64) -> Vec<Sample> {
    let rules = StandardConga::default();
    let starting_dealer = Seat::new(0);
    let mut out = generate_match(&rules, a_name, b_name, starting_dealer, seed);
    out.extend(generate_match(
        &rules,
        b_name,
        a_name,
        starting_dealer,
        seed,
    ));
    out
}

fn generate_match(
    rules: &StandardConga,
    agent_seat0: &str,
    agent_seat1: &str,
    starting_dealer: Seat,
    seed: u64,
) -> Vec<Sample> {
    // Distinct seeds per agent instance so same-agent paired matches don't
    // collide. Matches the eval harness pattern.
    let mut a = BaselineAgent::from_name(agent_seat0, seed ^ 0xA11A_B011_B011_A11A)
        .expect("agent name must be known");
    let mut b = BaselineAgent::from_name(agent_seat1, seed ^ 0xB022_B022_B022_B022)
        .expect("agent name must be known");
    let mut tracker0 = BeliefTracker::new(Seat::new(0), 2);
    let mut tracker1 = BeliefTracker::new(Seat::new(1), 2);

    let mut match_state = MatchState::new(2, starting_dealer);
    let mut chance = RandomChance::new(ChaCha8Rng::seed_from_u64(seed));

    let mut hand_samples: Vec<([f32; ENCODING_SIZE], Seat)> = Vec::new();
    let mut final_samples: Vec<Sample> = Vec::new();
    let mut legal_buf: Vec<Action> = Vec::new();
    let mut events_buf: Vec<Event> = Vec::new();

    while !match_state.is_finished() {
        let mut state = rules.new_hand(&match_state.scores, match_state.dealer, &mut chance);
        hand_samples.clear();

        loop {
            if let Some(result) = rules.hand_result(&state) {
                finalize_hand_samples(
                    &mut hand_samples,
                    &match_state.scores,
                    &result,
                    &mut final_samples,
                );
                match_state.finish_hand(rules, &result);
                deliver_hand_ended(&mut a, &mut b, &mut tracker0, &mut tracker1, &result);
                break;
            }

            let peek = rules.observe(&state, Seat::new(0));
            let current = peek.current_seat;
            let obs = if current.index() == 0 {
                peek
            } else {
                rules.observe(&state, current)
            };

            legal_buf.clear();
            rules.legal_actions(&state, &mut legal_buf);

            if matches!(obs.phase, Phase::Draw | Phase::Discard) {
                let tracker = if current.index() == 0 {
                    &tracker0
                } else {
                    &tracker1
                };
                let enc = encode(&obs, tracker);
                hand_samples.push((enc, current));
            }

            let action = match current.index() {
                0 => a.act(&obs, &legal_buf),
                1 => b.act(&obs, &legal_buf),
                _ => unreachable!("dataset generator only supports 2 seats"),
            };

            events_buf.clear();
            rules.apply(&mut state, action, &mut chance, &mut events_buf);
            for ev in &events_buf {
                a.notify(ev);
                b.notify(ev);
                tracker0.notify(ev);
                tracker1.notify(ev);
            }
        }
    }

    debug_assert!(matches!(match_state.status, MatchStatus::Winner(_)));
    final_samples
}

fn finalize_hand_samples(
    hand_samples: &mut Vec<([f32; ENCODING_SIZE], Seat)>,
    pre_hand_scores: &[i32],
    result: &HandResult,
    out: &mut Vec<Sample>,
) {
    let table = committed_table();
    for (encoding, seat) in hand_samples.drain(..) {
        let s = seat.index() as usize;
        let other = 1 - s;
        let own_total = pre_hand_scores[s] + result.points[s];
        let opp_total = pre_hand_scores[other] + result.points[other];
        let label = table.lookup(own_total, opp_total);
        out.push(Sample { encoding, label });
    }
}

fn deliver_hand_ended(
    a: &mut BaselineAgent,
    b: &mut BaselineAgent,
    tracker0: &mut BeliefTracker,
    tracker1: &mut BeliefTracker,
    result: &HandResult,
) {
    let evt = Event::HandEnded(result.clone());
    a.notify(&evt);
    b.notify(&evt);
    tracker0.notify(&evt);
    tracker1.notify(&evt);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = env::temp_dir();
        p.push(format!(
            "conga-ai-dataset-{}-{}.bin",
            std::process::id(),
            name
        ));
        p
    }

    fn mk_sample(seed: u8) -> Sample {
        let mut encoding = [0.0f32; ENCODING_SIZE];
        for (i, slot) in encoding.iter_mut().enumerate() {
            *slot = (i as u8 ^ seed) as f32 / 255.0;
        }
        Sample {
            encoding,
            label: seed as f32 / 255.0,
        }
    }

    #[test]
    fn write_read_roundtrip() {
        let samples: Vec<Sample> = (0..10).map(|i| mk_sample(i as u8)).collect();
        let path = tmp_path("roundtrip");
        write_binary(&path, &samples).unwrap();
        let read = read_binary(&path).unwrap();
        assert_eq!(read, samples);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn read_rejects_bad_magic() {
        let path = tmp_path("bad_magic");
        std::fs::write(&path, b"NOPE0000AAAAAAAABBBBBBBB").unwrap();
        let err = read_binary(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn header_bytes_constant_matches_layout() {
        assert_eq!(HEADER_BYTES, 8 + 4 + 4 + 8);
        assert_eq!(ROW_BYTES, (ENCODING_SIZE + 1) * 4);
    }

    #[test]
    fn generate_smoke_is_nonempty_with_finite_labels() {
        let samples = generate(1, 0);
        assert!(!samples.is_empty(), "generate(1, 0) must produce samples");
        for s in &samples {
            assert!(
                s.label.is_finite() && (0.0..=1.0).contains(&s.label),
                "label {} is not a valid equity",
                s.label
            );
            for &v in &s.encoding {
                assert!(v.is_finite(), "encoding slot not finite: {v}");
            }
        }
    }

    #[test]
    fn generate_excludes_showdown_decisions() {
        let samples = generate(1, 1);
        assert!(!samples.is_empty());
        for s in &samples {
            // Phase block in the encoding: slot 250 is the Showdown
            // one-hot (D53). The generator must never record a Showdown
            // decision — all samples carry Draw (248) or Discard (249).
            assert_eq!(
                s.encoding[Sample::PHASE_SHOWDOWN_OFFSET],
                0.0,
                "sample recorded during Showdown phase"
            );
            let phase_sum = s.encoding[248] + s.encoding[249] + s.encoding[250];
            assert!(
                (phase_sum - 1.0).abs() < 1e-6,
                "phase one-hot must sum to 1.0, got {phase_sum}"
            );
        }
    }

    #[test]
    fn generate_is_deterministic() {
        let a = generate(1, 7);
        let b = generate(1, 7);
        assert_eq!(a, b, "same seed must produce byte-identical samples");
    }
}
