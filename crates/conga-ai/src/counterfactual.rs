//! Counterfactual probes (D46): replay a seeded match up to a target
//! decision, snapshot the position, then systematically perturb one
//! input (match scores, choice of close-card) and record whether
//! `ScoreAwarePlayer` picks a different action.
//!
//! Scope for m6:
//! - **Score perturbations.** Own and opponent scores at ±5 / ±10 points
//!   (clamped). These answer "does the match-score context change the
//!   choice?" — directly relevant to design-doc question 2.
//! - **Close-card swaps.** When the original action is `Close(c)`, every
//!   other legal `Close(c')` is logged with its post-close equity so
//!   the per-action value is visible alongside the picked one. This is
//!   more "per-action values" than strict counterfactual (the input
//!   is unchanged), but it falls out of the same replay for free.
//!
//! Belief perturbations ("remove this known opponent card") are
//! deferred; they need the agent's internal belief to be re-perturbed
//! per probe, which lands with slice 4's candidate wrapper.

use std::io;

use conga_core::{
    Action, Card, Event, Observation, Phase, RandomChance, RuleSet, Seat, best_meld_split,
};
use conga_core::{Hand, MeldParams};
use conga_rules::StandardConga;
use conga_server::MatchState;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::belief::BeliefTracker;
use crate::disagreement::action_label;
use crate::equity::{EquityTable, committed_table};
use crate::player::Player;
use crate::score_aware::ScoreAwarePlayer;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterfactualRow {
    pub seed: u64,
    pub hand: u32,
    pub turn: u32,
    pub current_seat: u8,
    pub perturbation: String,
    pub original_action: String,
    pub new_action: String,
    pub action_changed: bool,
    pub equity_original: f32,
    pub equity_new: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterfactualSweep {
    pub rows: Vec<CounterfactualRow>,
    pub positions_probed: u32,
    pub positions_requested: u32,
    pub positions_missing: Vec<ProbeTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProbeTarget {
    pub seed: u64,
    pub hand: u32,
    pub turn: u32,
    pub current_seat: u8,
    pub phase: Phase,
}

/// Probe every target in `targets` and collect the results into one
/// [`CounterfactualSweep`]. Targets that cannot be reconstructed (the
/// seeded match never reaches the requested decision) are logged in
/// `positions_missing` rather than silently dropped.
pub fn sweep(targets: &[ProbeTarget], sa_samples: u32) -> CounterfactualSweep {
    let mut rows = Vec::new();
    let mut probed = 0u32;
    let mut missing = Vec::new();
    for t in targets {
        match probe_position(t.seed, t.hand, t.turn, t.current_seat, t.phase, sa_samples) {
            Some(mut chunk) => {
                probed += 1;
                rows.append(&mut chunk);
            }
            None => missing.push(*t),
        }
    }
    CounterfactualSweep {
        rows,
        positions_probed: probed,
        positions_requested: targets.len() as u32,
        positions_missing: missing,
    }
}

/// Replay a seeded score-aware vs score-aware match, snapshot the
/// state at `(seed, hand, turn, current_seat)` in `Draw`/`Discard`
/// phase, and run every supported perturbation at that snapshot.
/// Returns `None` when the replay never reaches the requested
/// position (e.g. the hand ended before the target turn).
pub fn probe_position(
    seed: u64,
    hand: u32,
    turn: u32,
    current_seat: u8,
    phase: Phase,
    sa_samples: u32,
) -> Option<Vec<CounterfactualRow>> {
    let snap = replay_to_target(seed, hand, turn, current_seat, phase, sa_samples)?;
    let mut probe_rng = ChaCha8Rng::seed_from_u64(seed ^ 0xC0FE_CA7E_C0FE_CA7Eu64);
    let mut rows = Vec::new();
    let rules = StandardConga::default();
    let table = committed_table();

    let ReplaySnapshot {
        mut sa_at_target,
        belief,
        obs,
        legal_actions,
        original_action,
    } = snap;

    let original_label = action_label(&original_action);
    let eq_original = equity_for_action(
        &original_action,
        &obs,
        &belief,
        &rules,
        &mut probe_rng,
        sa_samples,
        table,
    );

    for delta in [-10i32, -5, 5, 10] {
        for who in [Who::Own, Who::Opp] {
            let modified_obs = modify_scores(&obs, who, delta);
            let new_action = sa_at_target.act(&modified_obs, &legal_actions);
            let eq_new = equity_for_action(
                &new_action,
                &modified_obs,
                &belief,
                &rules,
                &mut probe_rng,
                sa_samples,
                table,
            );
            rows.push(CounterfactualRow {
                seed,
                hand,
                turn,
                current_seat,
                perturbation: format!("{who}_score{delta:+}"),
                original_action: original_label.clone(),
                new_action: action_label(&new_action),
                action_changed: new_action != original_action,
                equity_original: eq_original,
                equity_new: eq_new,
            });
        }
    }

    if let Action::Close(orig_c) = original_action {
        for a in &legal_actions {
            if let Action::Close(c) = a {
                if *c == orig_c {
                    continue;
                }
                let alt = Action::Close(*c);
                let eq_alt = equity_for_action(
                    &alt,
                    &obs,
                    &belief,
                    &rules,
                    &mut probe_rng,
                    sa_samples,
                    table,
                );
                rows.push(CounterfactualRow {
                    seed,
                    hand,
                    turn,
                    current_seat,
                    perturbation: format!("swap_close({c})"),
                    original_action: original_label.clone(),
                    new_action: action_label(&alt),
                    action_changed: true,
                    equity_original: eq_original,
                    equity_new: eq_alt,
                });
            }
        }
    }

    Some(rows)
}

#[derive(Debug, Clone, Copy)]
enum Who {
    Own,
    Opp,
}

impl std::fmt::Display for Who {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Who::Own => "own",
            Who::Opp => "opp",
        })
    }
}

fn modify_scores(obs: &Observation, who: Who, delta: i32) -> Observation {
    let mut out = obs.clone();
    let idx = match who {
        Who::Own => obs.own_seat.index() as usize,
        Who::Opp => ((obs.own_seat.index() + 1) % obs.hand_sizes.len() as u8) as usize,
    };
    out.scores[idx] = (out.scores[idx] + delta).clamp(-50, 110);
    out
}

struct ReplaySnapshot {
    sa_at_target: ScoreAwarePlayer,
    belief: BeliefTracker,
    obs: Observation,
    legal_actions: Vec<Action>,
    original_action: Action,
}

fn replay_to_target(
    seed: u64,
    hand: u32,
    turn: u32,
    current_seat: u8,
    phase: Phase,
    sa_samples: u32,
) -> Option<ReplaySnapshot> {
    let rules = StandardConga::default();
    let mut sa0 = ScoreAwarePlayer::new(sa_samples, seed.wrapping_mul(2));
    let mut sa1 = ScoreAwarePlayer::new(sa_samples, seed.wrapping_mul(2).wrapping_add(1));
    let mut belief = [
        BeliefTracker::new(Seat::new(0), 2),
        BeliefTracker::new(Seat::new(1), 2),
    ];
    let mut chance = RandomChance::new(ChaCha8Rng::seed_from_u64(seed));
    let mut match_state = MatchState::new(2, Seat::new(0));
    let mut actions_buf: Vec<Action> = Vec::new();
    let mut events_buf: Vec<Event> = Vec::new();
    let mut hand_idx: u32 = 0;

    while !match_state.is_finished() {
        let mut state = rules.new_hand(&match_state.scores, match_state.dealer, &mut chance);
        loop {
            if let Some(result) = rules.hand_result(&state) {
                let evt = Event::HandEnded(result.clone());
                sa0.notify(&evt);
                sa1.notify(&evt);
                for b in belief.iter_mut() {
                    b.notify(&evt);
                }
                match_state.finish_hand(&rules, &result);
                hand_idx += 1;
                if hand_idx > hand {
                    return None;
                }
                break;
            }
            let current = state.current_seat;
            let obs = rules.observe(&state, current);
            actions_buf.clear();
            rules.legal_actions(&state, &mut actions_buf);

            let is_target = hand_idx == hand
                && current.index() == current_seat
                && obs.discard_history.len() as u32 == turn
                && obs.phase == phase
                && matches!(phase, Phase::Draw | Phase::Discard);
            if is_target {
                let mut sa_at_target = if current.index() == 0 { sa0 } else { sa1 };
                let original_action = sa_at_target.act(&obs, &actions_buf);
                return Some(ReplaySnapshot {
                    sa_at_target,
                    belief: belief[current.index() as usize].clone(),
                    obs: obs.clone(),
                    legal_actions: actions_buf.clone(),
                    original_action,
                });
            }

            let seat_idx = current.index() as usize;
            let action = if seat_idx == 0 {
                sa0.act(&obs, &actions_buf)
            } else {
                sa1.act(&obs, &actions_buf)
            };

            events_buf.clear();
            rules.apply(&mut state, action, &mut chance, &mut events_buf);
            for e in &events_buf {
                sa0.notify(e);
                sa1.notify(e);
                for b in belief.iter_mut() {
                    b.notify(e);
                }
            }
        }
    }
    None
}

/// Equity lookup for an `action` at `obs`. Mirrors
/// [`crate::disagreement::equity_for_action`] but is local because
/// that one is a private helper; keep both in sync. For `Close(c)`
/// the value is the expected post-close equity averaged over
/// `samples` belief draws; for every other action it is the
/// status-quo table lookup.
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

/// Write a [`CounterfactualSweep`] to `w` as pretty-printed JSON.
pub fn write_json<W: io::Write>(sweep: &CounterfactualSweep, w: &mut W) -> io::Result<()> {
    serde_json::to_writer_pretty(w, sweep).map_err(|e| io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disagreement::mine_with_samples;

    #[test]
    fn probe_at_first_disagreement_emits_rows() {
        // Mine one seed to find a reachable target, then probe it.
        let rows = mine_with_samples(&[0xC0FFEEu64], 1, 4);
        assert!(!rows.is_empty());
        let r = &rows[0];
        let out = probe_position(r.seed, r.hand, r.turn, r.current_seat, r.phase, 4)
            .expect("replay should reach a position just mined");
        // 8 score perturbations + close-swap alternatives (>=0).
        assert!(out.len() >= 8);
        let has_score = out.iter().any(|row| row.perturbation.contains("_score"));
        assert!(has_score);
    }

    #[test]
    fn probe_missing_position_returns_none() {
        // Impossible target: turn 999 never happens.
        let out = probe_position(0, 0, 999, 0, Phase::Discard, 4);
        assert!(out.is_none());
    }

    #[test]
    fn sweep_logs_missing_positions() {
        let targets = vec![
            ProbeTarget {
                seed: 0,
                hand: 0,
                turn: 999,
                current_seat: 0,
                phase: Phase::Discard,
            },
            ProbeTarget {
                seed: 0,
                hand: 0,
                turn: 1000,
                current_seat: 1,
                phase: Phase::Discard,
            },
        ];
        let out = sweep(&targets, 4);
        assert_eq!(out.positions_probed, 0);
        assert_eq!(out.positions_requested, 2);
        assert_eq!(out.positions_missing.len(), 2);
    }

    #[test]
    fn close_swap_rows_only_appear_when_original_is_close() {
        // At a Close disagreement with at least two legal Close
        // options the probe should surface each alternative under
        // `swap_close(...)`. Walk the mined disagreements until we
        // find one that satisfies both conditions — not every mined
        // Close row has >1 legal Close (hand composition sometimes
        // leaves a unique legal close).
        let rows = mine_with_samples(&(0..5u64).collect::<Vec<_>>(), 3, 4);
        let mut found_swap = false;
        for r in &rows {
            if !matches!(r.score_aware_action, Action::Close(_)) {
                continue;
            }
            let out = probe_position(r.seed, r.hand, r.turn, r.current_seat, r.phase, 4).unwrap();
            let swap_count = out
                .iter()
                .filter(|row| row.perturbation.starts_with("swap_close"))
                .count();
            if swap_count > 0 {
                found_swap = true;
                break;
            }
        }
        assert!(
            found_swap,
            "expected at least one mined Close disagreement to have >=2 legal Close options"
        );
    }

    #[test]
    fn score_perturbation_within_bounds() {
        let rows = mine_with_samples(&[0xC0FFEEu64], 1, 4);
        let r = &rows[0];
        let out = probe_position(r.seed, r.hand, r.turn, r.current_seat, r.phase, 4).unwrap();
        for row in &out {
            if row.perturbation.contains("_score") {
                assert!(row.equity_new >= 0.0 && row.equity_new <= 1.0);
                assert!(row.equity_original >= 0.0 && row.equity_original <= 1.0);
            }
        }
    }

    #[test]
    fn sweep_json_roundtrip() {
        let rows = mine_with_samples(&[0xC0FFEEu64], 1, 4);
        let r = &rows[0];
        let targets = vec![ProbeTarget {
            seed: r.seed,
            hand: r.hand,
            turn: r.turn,
            current_seat: r.current_seat,
            phase: r.phase,
        }];
        let out = sweep(&targets, 4);
        let mut buf: Vec<u8> = Vec::new();
        write_json(&out, &mut buf).unwrap();
        let back: CounterfactualSweep = serde_json::from_slice(&buf).unwrap();
        assert_eq!(back.rows.len(), out.rows.len());
        assert_eq!(back.positions_probed, out.positions_probed);
    }
}
