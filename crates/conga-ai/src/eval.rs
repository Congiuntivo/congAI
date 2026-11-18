//! Headless evaluation: play agents against each other without the
//! server/transport layer. The engine is driven directly via
//! `rules.legal_actions` → `agent.act` → `rules.apply`, which is fast
//! enough that the slice-6 regression tests can run thousands of
//! matches in a few seconds (D29).

use conga_core::{Action, Event, HandResult, MatchStatus, RandomChance, RuleSet, Seat};
use conga_server::MatchState;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

use crate::player::Player;

/// Final state of one head-to-head match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchOutcome {
    pub winner_seat: Option<Seat>,
    pub final_scores: Vec<i32>,
    pub hands_played: u32,
    /// Total number of `act` calls across both seats during the match.
    pub decisions: u64,
}

impl MatchOutcome {
    /// `true` if the seat-0 player won this match.
    pub fn seat_0_won(&self) -> bool {
        self.winner_seat == Some(Seat::new(0))
    }
}

/// Outcome of a pair of matches on the same seed with seats swapped.
/// `a_on_seat_0` is the match where builder A played seat 0 and B
/// played seat 1; `a_on_seat_1` is the mirror.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedResult {
    pub a_on_seat_0: MatchOutcome,
    pub a_on_seat_1: MatchOutcome,
}

impl PairedResult {
    /// Number of matches in this pair that agent A won (0, 1 or 2).
    pub fn a_wins(&self) -> u32 {
        let mut w = 0;
        if self.a_on_seat_0.winner_seat == Some(Seat::new(0)) {
            w += 1;
        }
        if self.a_on_seat_1.winner_seat == Some(Seat::new(1)) {
            w += 1;
        }
        w
    }

    /// Number of matches in this pair that agent B won.
    pub fn b_wins(&self) -> u32 {
        let mut w = 0;
        if self.a_on_seat_0.winner_seat == Some(Seat::new(1)) {
            w += 1;
        }
        if self.a_on_seat_1.winner_seat == Some(Seat::new(0)) {
            w += 1;
        }
        w
    }
}

/// Play one match head-to-head: seat 0 is driven by `a`, seat 1 by
/// `b`. Returns a `MatchOutcome` once `match_status` reports a winner.
pub fn run_match<R, A, B>(
    rules: &R,
    a: &mut A,
    b: &mut B,
    starting_dealer: Seat,
    seed: u64,
) -> MatchOutcome
where
    R: RuleSet,
    A: Player,
    B: Player,
{
    let mut chance = RandomChance::new(ChaCha8Rng::seed_from_u64(seed));
    let seats: u8 = 2;
    let mut match_state = MatchState::new(seats, starting_dealer);
    let mut decisions: u64 = 0;
    let mut actions_buf: Vec<Action> = Vec::new();
    let mut events_buf: Vec<Event> = Vec::new();

    while !match_state.is_finished() {
        let mut state = rules.new_hand(&match_state.scores, match_state.dealer, &mut chance);

        loop {
            if let Some(result) = rules.hand_result(&state) {
                match_state.finish_hand(rules, &result);
                deliver_hand_ended(a, b, &result);
                break;
            }

            // Peek the current seat via a single observation then build
            // the real one from the seat's perspective.
            let peek = rules.observe(&state, Seat::new(0));
            let current = peek.current_seat;
            let obs = if current.index() == 0 {
                peek
            } else {
                rules.observe(&state, current)
            };

            actions_buf.clear();
            rules.legal_actions(&state, &mut actions_buf);
            assert!(
                !actions_buf.is_empty(),
                "engine produced no legal actions mid-hand"
            );

            let action = match current.index() {
                0 => a.act(&obs, &actions_buf),
                1 => b.act(&obs, &actions_buf),
                _ => unreachable!("run_match only supports 2 seats"),
            };
            decisions += 1;

            events_buf.clear();
            rules.apply(&mut state, action, &mut chance, &mut events_buf);

            for event in &events_buf {
                a.notify(event);
                b.notify(event);
            }
        }
    }

    let winner_seat = match match_state.status {
        MatchStatus::Winner(w) => Some(w),
        MatchStatus::InProgress => None,
    };

    MatchOutcome {
        winner_seat,
        final_scores: match_state.scores,
        hands_played: match_state.hands_played,
        decisions,
    }
}

fn deliver_hand_ended<A: Player, B: Player>(a: &mut A, b: &mut B, result: &HandResult) {
    let evt = Event::HandEnded(result.clone());
    a.notify(&evt);
    b.notify(&evt);
}

/// Play the same match twice with the same seed, swapping seats
/// between agent A and agent B. Each call to the builders yields a
/// fresh player instance so cross-match state does not leak.
pub fn run_paired<R, FA, FB, A, B>(
    rules: &R,
    mut builder_a: FA,
    mut builder_b: FB,
    starting_dealer: Seat,
    seed: u64,
) -> PairedResult
where
    R: RuleSet,
    FA: FnMut() -> A,
    FB: FnMut() -> B,
    A: Player,
    B: Player,
{
    let mut a1 = builder_a();
    let mut b1 = builder_b();
    let m1 = run_match(rules, &mut a1, &mut b1, starting_dealer, seed);

    let mut a2 = builder_a();
    let mut b2 = builder_b();
    // On the second match, agent B plays seat 0 and agent A plays
    // seat 1 — i.e. the seats swap.
    let m2 = run_match(rules, &mut b2, &mut a2, starting_dealer, seed);

    PairedResult {
        a_on_seat_0: m1,
        a_on_seat_1: m2,
    }
}

/// Play `pairs` paired matches using consecutive seeds starting at
/// `start_seed`. Total matches = `2 * pairs`.
pub fn run_batch<R, FA, FB, A, B>(
    rules: &R,
    mut builder_a: FA,
    mut builder_b: FB,
    pairs: u32,
    start_seed: u64,
    starting_dealer: Seat,
) -> Vec<PairedResult>
where
    R: RuleSet,
    FA: FnMut() -> A,
    FB: FnMut() -> B,
    A: Player,
    B: Player,
{
    let mut out = Vec::with_capacity(pairs as usize);
    for i in 0..pairs {
        let seed = start_seed.wrapping_add(i as u64);
        let pair = run_paired(rules, &mut builder_a, &mut builder_b, starting_dealer, seed);
        out.push(pair);
    }
    out
}
