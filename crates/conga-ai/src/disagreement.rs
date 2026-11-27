//! Disagreement mining: positions where `ScoreAwarePlayer` and
//! `RuleBasedPlayer` pick different actions, with their features and
//! their post-action equity (D46).
//!
//! A score-aware vs score-aware match is driven for each seed. At each
//! `Draw` / `Discard` decision a shadow `RuleBasedPlayer` for the current
//! seat is asked what it would do with the same observation; when its
//! action differs from the driver's, a [`DisagreementRow`] is recorded.
//! The shadow receives every event alongside the driver so its internal
//! belief stays consistent.
//!
//! For `Close` actions the equity column reflects the post-close score
//! pair, averaged over K belief samples. For every other action it is
//! the status-quo lookup — not strictly the equity *after* that action,
//! but any two non-`Close` actions share the same status-quo position
//! so the column still separates by action family.

use std::io::{self, Write};

use conga_core::{
    Action, Card, Event, Hand, MeldParams, Observation, Phase, RandomChance, RuleSet, Seat,
    best_meld_split,
};
use conga_rules::StandardConga;
use conga_server::MatchState;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::belief::BeliefTracker;
use crate::equity::{EquityTable, committed_table};
use crate::features::{Features, extract as extract_features};
use crate::player::Player;
use crate::rule_based::RuleBasedPlayer;
use crate::score_aware::{DEFAULT_SCORE_AWARE_SAMPLES, ScoreAwarePlayer};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisagreementRow {
    pub seed: u64,
    pub turn: u32,
    pub current_seat: u8,
    pub phase: Phase,
    pub features: Features,
    pub score_aware_action: Action,
    pub rule_based_action: Action,
    pub equity_at_score_aware_action: f32,
    pub equity_at_rule_based_action: f32,
}

/// Mine disagreements between `ScoreAwarePlayer` and `RuleBasedPlayer`
/// across a list of seeds. At most `max_per_seed` rows are kept per
/// seed; once that cap is hit the seed's match loop breaks early.
pub fn mine_disagreements(seeds: &[u64], max_per_seed: usize) -> Vec<DisagreementRow> {
    mine_with_samples(seeds, max_per_seed, DEFAULT_SCORE_AWARE_SAMPLES)
}

/// Same as [`mine_disagreements`] but with a tunable belief-sample
/// count for both the driver's close-card choice and the per-row
/// post-close equity estimate. Lower values speed the tests up.
pub fn mine_with_samples(
    seeds: &[u64],
    max_per_seed: usize,
    sa_samples: u32,
) -> Vec<DisagreementRow> {
    let rules = StandardConga::default();
    let table = committed_table();
    let mut out: Vec<DisagreementRow> = Vec::new();
    let mut actions_buf: Vec<Action> = Vec::new();
    let mut events_buf: Vec<Event> = Vec::new();

    for &seed in seeds {
        let mut sa = [
            ScoreAwarePlayer::new(sa_samples, seed.wrapping_mul(2)),
            ScoreAwarePlayer::new(sa_samples, seed.wrapping_mul(2).wrapping_add(1)),
        ];
        let mut rb = [RuleBasedPlayer::new(), RuleBasedPlayer::new()];
        let mut belief = [
            BeliefTracker::new(Seat::new(0), 2),
            BeliefTracker::new(Seat::new(1), 2),
        ];
        let mut row_rng = ChaCha8Rng::seed_from_u64(seed ^ 0xD15A_1EED_D15A_1EEDu64);
        let mut chance = RandomChance::new(ChaCha8Rng::seed_from_u64(seed));
        let mut match_state = MatchState::new(2, Seat::new(0));
        let mut collected_this_seed = 0usize;

        'match_loop: while !match_state.is_finished() {
            let mut state = rules.new_hand(&match_state.scores, match_state.dealer, &mut chance);
            loop {
                if let Some(result) = rules.hand_result(&state) {
                    let evt = Event::HandEnded(result.clone());
                    for p in sa.iter_mut() {
                        p.notify(&evt);
                    }
                    for p in rb.iter_mut() {
                        p.notify(&evt);
                    }
                    for b in belief.iter_mut() {
                        b.notify(&evt);
                    }
                    match_state.finish_hand(&rules, &result);
                    break;
                }

                let current = state.current_seat;
                let obs = rules.observe(&state, current);
                let seat_idx = current.index() as usize;

                actions_buf.clear();
                rules.legal_actions(&state, &mut actions_buf);

                let sa_action = sa[seat_idx].act(&obs, &actions_buf);

                if matches!(obs.phase, Phase::Draw | Phase::Discard) {
                    let rb_action = rb[seat_idx].act(&obs, &actions_buf);
                    if sa_action != rb_action {
                        let features = extract_features(&obs, &belief[seat_idx], &rules);
                        let eq_sa = equity_for_action(
                            &sa_action,
                            &obs,
                            &belief[seat_idx],
                            &rules,
                            &mut row_rng,
                            sa_samples,
                            table,
                        );
                        let eq_rb = equity_for_action(
                            &rb_action,
                            &obs,
                            &belief[seat_idx],
                            &rules,
                            &mut row_rng,
                            sa_samples,
                            table,
                        );
                        out.push(DisagreementRow {
                            seed,
                            turn: obs.discard_history.len() as u32,
                            current_seat: current.index(),
                            phase: obs.phase,
                            features,
                            score_aware_action: sa_action.clone(),
                            rule_based_action: rb_action,
                            equity_at_score_aware_action: eq_sa,
                            equity_at_rule_based_action: eq_rb,
                        });
                        collected_this_seed += 1;
                        if collected_this_seed >= max_per_seed {
                            break 'match_loop;
                        }
                    }
                }

                events_buf.clear();
                rules.apply(&mut state, sa_action, &mut chance, &mut events_buf);
                for e in &events_buf {
                    for p in sa.iter_mut() {
                        p.notify(e);
                    }
                    for p in rb.iter_mut() {
                        p.notify(e);
                    }
                    for b in belief.iter_mut() {
                        b.notify(e);
                    }
                }
            }
        }
    }

    out
}

/// Equity lookup for an `action` at `obs`. For `Close(c)` this is the
/// expected post-close equity averaged over `samples` belief draws; for
/// every other action it is the status-quo table lookup at
/// `(own_score, opp_score)`.
fn equity_for_action(
    action: &Action,
    obs: &Observation,
    belief: &BeliefTracker,
    rules: &StandardConga,
    rng: &mut dyn Rng,
    samples: u32,
    table: &EquityTable,
) -> f32 {
    let own_idx = obs.own_seat.index() as usize;
    let opp = Seat::new((obs.own_seat.index() + 1) % 2);
    let opp_idx = opp.index() as usize;
    let own_score = obs.scores[own_idx];
    let opp_score = obs.scores[opp_idx];

    match action {
        Action::Close(c) => {
            let mut hand_after = obs.own_hand;
            hand_after.remove(*c);
            let own_delta = own_close_delta(hand_after);
            let new_own = own_score + own_delta;
            let s = samples.max(1);
            let mut sum = 0.0f64;
            for _ in 0..s {
                let sampled = belief.sample_state(rng, obs, rules);
                let opp_hand = sampled.hands[opp_idx];
                let opp_res =
                    best_meld_split(opp_hand, &MeldParams::standard()).unmatched_total as i32;
                sum += table.lookup(new_own, opp_score + opp_res) as f64;
            }
            (sum / s as f64) as f32
        }
        _ => table.lookup(own_score, opp_score),
    }
}

/// Mirror of `ScoreAwarePlayer::own_close_delta` kept local to avoid
/// widening the `score_aware` module's public surface. Keep both in
/// sync: a clean split pays -10, a 7-card single-suit run pays -50.
fn own_close_delta(hand_after_close: Hand) -> i32 {
    let meld = MeldParams::standard();
    let split = best_meld_split(hand_after_close, &meld);
    let base = split.unmatched_total as i32;
    if is_conga(hand_after_close) {
        base - 50
    } else if split.unmatched.is_empty() {
        base - 10
    } else {
        base
    }
}

fn is_conga(hand: Hand) -> bool {
    if hand.len() != 7 {
        return false;
    }
    let cards: Vec<Card> = hand.iter().collect();
    let suit = cards[0].suit();
    if !cards.iter().all(|c| c.suit() == suit) {
        return false;
    }
    let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank().as_u8()).collect();
    ranks.sort_unstable();
    ranks.windows(2).all(|w| w[1] == w[0] + 1)
}

/// Flat row shape used for CSV read/write. The nested `Features`
/// struct is lifted so every column is a primitive or a plain string;
/// `Action` columns are rendered via [`action_label`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CsvRow {
    seed: u64,
    turn: u32,
    current_seat: u8,
    phase: String,
    unmatched_total: u32,
    unmatched_count: u8,
    meld_count: u8,
    can_close: bool,
    close_improver_count: u8,
    own_distance_to_elim: i32,
    opp_distance_to_elim: i32,
    top_discard: String,
    turn_number: u32,
    known_opp_cards_count: u8,
    score_aware_action: String,
    rule_based_action: String,
    equity_at_score_aware_action: f32,
    equity_at_rule_based_action: f32,
}

impl From<&DisagreementRow> for CsvRow {
    fn from(r: &DisagreementRow) -> Self {
        CsvRow {
            seed: r.seed,
            turn: r.turn,
            current_seat: r.current_seat,
            phase: phase_str(r.phase).to_string(),
            unmatched_total: r.features.unmatched_total,
            unmatched_count: r.features.unmatched_count,
            meld_count: r.features.meld_count,
            can_close: r.features.can_close,
            close_improver_count: r.features.close_improver_count,
            own_distance_to_elim: r.features.own_distance_to_elim,
            opp_distance_to_elim: r.features.opp_distance_to_elim,
            top_discard: r
                .features
                .top_discard
                .map(|c| c.to_string())
                .unwrap_or_default(),
            turn_number: r.features.turn_number,
            known_opp_cards_count: r.features.known_opp_cards_count,
            score_aware_action: action_label(&r.score_aware_action),
            rule_based_action: action_label(&r.rule_based_action),
            equity_at_score_aware_action: r.equity_at_score_aware_action,
            equity_at_rule_based_action: r.equity_at_rule_based_action,
        }
    }
}

/// Write the rows as CSV via `csv::Writer`. Actions are rendered as
/// display strings so the file opens in a spreadsheet without
/// post-processing; the structured `DisagreementRow` stays available
/// programmatically.
pub fn write_csv<W: Write>(rows: &[DisagreementRow], w: &mut W) -> io::Result<()> {
    let mut wtr = csv::Writer::from_writer(w);
    for r in rows {
        wtr.serialize(CsvRow::from(r))
            .map_err(|e| io::Error::other(e.to_string()))?;
    }
    wtr.flush()
}

/// Read a CSV written by [`write_csv`] into a list of flat `CsvRow`
/// records. Slice-3 counterfactual-sweep consumes these rows to pick
/// positions to probe; the full `DisagreementRow` is not reconstructed
/// because the position (seed, turn, seat) is enough to replay back to
/// the right state.
pub fn read_csv_rows<R: std::io::Read>(r: R) -> io::Result<Vec<CsvRecord>> {
    let mut rdr = csv::Reader::from_reader(r);
    let mut out = Vec::new();
    for res in rdr.deserialize::<CsvRow>() {
        let row = res.map_err(|e| io::Error::other(e.to_string()))?;
        out.push(CsvRecord {
            seed: row.seed,
            turn: row.turn,
            current_seat: row.current_seat,
            phase: row.phase,
            score_aware_action: row.score_aware_action,
            rule_based_action: row.rule_based_action,
            equity_at_score_aware_action: row.equity_at_score_aware_action,
            equity_at_rule_based_action: row.equity_at_rule_based_action,
        });
    }
    Ok(out)
}

/// Compact replay-friendly view of a CSV row. Slice 3 uses `(seed,
/// turn, current_seat)` to drive a match back to the right decision
/// point and reconstructs features / belief from the observation then.
#[derive(Debug, Clone, PartialEq)]
pub struct CsvRecord {
    pub seed: u64,
    pub turn: u32,
    pub current_seat: u8,
    pub phase: String,
    pub score_aware_action: String,
    pub rule_based_action: String,
    pub equity_at_score_aware_action: f32,
    pub equity_at_rule_based_action: f32,
}

fn phase_str(p: Phase) -> &'static str {
    match p {
        Phase::Draw => "Draw",
        Phase::Discard => "Discard",
        Phase::Showdown => "Showdown",
        Phase::Ended => "Ended",
    }
}

pub fn action_label(a: &Action) -> String {
    match a {
        Action::DrawStock => "DrawStock".to_string(),
        Action::DrawDiscard => "DrawDiscard".to_string(),
        Action::Discard(c) => format!("Discard({c})"),
        Action::Close(c) => format!("Close({c})"),
        Action::LayMeld(_) => "LayMeld".to_string(),
        Action::AddToMeld { card, meld_index } => format!("AddToMeld({card};{meld_index})"),
        Action::Pass => "Pass".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_seed_surfaces_at_least_one_disagreement() {
        // Low belief-sample count for test speed; max_per_seed=1 means
        // the match breaks the moment a disagreement lands so this
        // stays fast even in dev builds.
        let rows = mine_with_samples(&[0xC0FFEEu64], 1, 4);
        assert!(
            !rows.is_empty(),
            "expected at least one disagreement in a full match"
        );
    }

    #[test]
    fn disagreement_row_json_roundtrip() {
        let rows = mine_with_samples(&[0xC0FFEEu64], 1, 4);
        assert!(!rows.is_empty());
        let json = serde_json::to_string(&rows[0]).unwrap();
        let back: DisagreementRow = serde_json::from_str(&json).unwrap();
        assert_eq!(rows[0], back);
    }

    #[test]
    fn csv_writer_emits_header_and_rows() {
        let rows = mine_with_samples(&[0xC0FFEEu64], 2, 4);
        assert!(!rows.is_empty());
        let mut buf: Vec<u8> = Vec::new();
        write_csv(&rows, &mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        let mut lines = text.lines();
        let header = lines.next().expect("header line");
        assert!(header.starts_with("seed,turn,current_seat,phase,"));
        assert!(header.ends_with(",equity_at_rule_based_action"));
        let data_lines: Vec<&str> = lines.collect();
        assert_eq!(data_lines.len(), rows.len());
    }

    #[test]
    fn csv_read_roundtrips_seed_turn_actions() {
        let rows = mine_with_samples(&[0xC0FFEEu64], 2, 4);
        assert!(!rows.is_empty());
        let mut buf: Vec<u8> = Vec::new();
        write_csv(&rows, &mut buf).unwrap();
        let records = read_csv_rows(buf.as_slice()).unwrap();
        assert_eq!(records.len(), rows.len());
        for (src, rec) in rows.iter().zip(records.iter()) {
            assert_eq!(rec.seed, src.seed);
            assert_eq!(rec.turn, src.turn);
            assert_eq!(rec.current_seat, src.current_seat);
            assert_eq!(
                rec.score_aware_action,
                action_label(&src.score_aware_action)
            );
            assert_eq!(rec.rule_based_action, action_label(&src.rule_based_action));
        }
    }

    #[test]
    fn action_label_formats_cards_as_rank_suit() {
        use conga_core::{Rank, Suit};
        let c = Card::new(Suit::Hearts, Rank::new(7).unwrap());
        assert_eq!(action_label(&Action::Close(c)), "Close(7H)");
        assert_eq!(action_label(&Action::DrawStock), "DrawStock");
        assert_eq!(action_label(&Action::Pass), "Pass");
    }

    #[test]
    #[ignore = "slow: runs 50 seeded matches with belief sampling"]
    fn fifty_seed_mine_produces_enough_rows() {
        let seeds: Vec<u64> = (0..50u64).collect();
        let rows = mine_with_samples(&seeds, 5, 8);
        assert!(
            rows.len() >= 10,
            "expected >=10 rows over 50 seeds, got {}",
            rows.len()
        );
    }
}
