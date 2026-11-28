//! Headless evaluation: play agents against each other without the
//! server/transport layer. The engine is driven directly via
//! `rules.legal_actions` → `agent.act` → `rules.apply`, which is fast
//! enough that the slice-6 regression tests can run thousands of
//! matches in a few seconds (D29).

use std::time::Duration;

use conga_core::{
    Action, Event, HandResult, MatchStatus, Observation, RandomChance, RuleSet, Seat,
};
use conga_server::MatchState;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

use crate::candidates::{CleanSplitCloseWrapper, DEFAULT_CLEAN_SPLIT_SAMPLES};
use crate::greedy::GreedyPlayer;
use crate::player::Player;
use crate::random::RandomPlayer;
use crate::rule_based::RuleBasedPlayer;
use crate::score_aware::{DEFAULT_SCORE_AWARE_SAMPLES, ScoreAwarePlayer};
use crate::search::SearchPlayer;

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

/// Play one match head-to-head from the default (all-zero) starting
/// scores: seat 0 is driven by `a`, seat 1 by `b`. Returns a
/// `MatchOutcome` once `match_status` reports a winner. Thin wrapper
/// over [`run_match_from`].
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
    run_match_from(rules, a, b, MatchState::new(2, starting_dealer), seed)
}

/// Play one match head-to-head starting from the given [`MatchState`]
/// (scores, dealer, status). Used by the m5 equity-table build to
/// sample matches that start from an arbitrary score pair. The caller
/// is responsible for passing a state whose `status` is `InProgress`;
/// a terminal state short-circuits to a vacuous outcome below.
pub fn run_match_from<R, A, B>(
    rules: &R,
    a: &mut A,
    b: &mut B,
    mut match_state: MatchState,
    seed: u64,
) -> MatchOutcome
where
    R: RuleSet,
    A: Player,
    B: Player,
{
    let mut chance = RandomChance::new(ChaCha8Rng::seed_from_u64(seed));
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

/// Enum over the baseline agents so the `conga-cli eval` dispatcher
/// (and tests) can pick one by name without monomorphising for every
/// (A, B) pair. Each variant wraps a concrete `Player`; the trait impl
/// just forwards.
pub enum BaselineAgent {
    // `RandomPlayer<ChaCha8Rng>` is ~304 bytes because ChaCha8 keeps a
    // block buffer; the other variants are tiny. Box it to keep the
    // enum compact and avoid clippy's large-variant warning.
    Random(Box<RandomPlayer<ChaCha8Rng>>),
    Greedy(GreedyPlayer),
    RuleBased(RuleBasedPlayer),
    Search(Box<SearchPlayer>),
    ScoreAware(Box<ScoreAwarePlayer>),
    CleanSplitClose(Box<CleanSplitCloseWrapper<RuleBasedPlayer>>),
}

/// Default number of belief-sampled worlds the search expands per act
/// call. Used by [`BaselineAgent::from_name`] when the caller does not
/// override it. See D33.
pub const DEFAULT_SEARCH_SAMPLES: u32 = 50;

impl BaselineAgent {
    /// `name` is one of `random`, `greedy`, `rule-based`, `search`, `score-aware`.
    /// `seed` drives the random agent's RNG and the search agent's
    /// belief sampling + rollout RNG. The search agent uses
    /// [`DEFAULT_SEARCH_SAMPLES`]; for a custom count see
    /// [`BaselineAgent::from_name_with_samples`].
    pub fn from_name(name: &str, seed: u64) -> Option<Self> {
        Self::from_name_with_samples(name, seed, DEFAULT_SEARCH_SAMPLES)
    }

    pub fn from_name_with_samples(name: &str, seed: u64, samples: u32) -> Option<Self> {
        match name {
            "random" => Some(BaselineAgent::Random(Box::new(RandomPlayer::from_seed(
                seed,
            )))),
            "greedy" => Some(BaselineAgent::Greedy(GreedyPlayer::new())),
            "rule-based" => Some(BaselineAgent::RuleBased(RuleBasedPlayer::new())),
            "search" => Some(BaselineAgent::Search(Box::new(SearchPlayer::new(
                samples, seed,
            )))),
            "score-aware" => {
                // The `samples` CLI flag doubles up: for `search` it's
                // the rollout count, for `score-aware` it's the Close-
                // gate belief samples. Use its value if it's not the
                // search default; otherwise use the score-aware
                // default so the common case stays sane.
                let k = if samples == DEFAULT_SEARCH_SAMPLES {
                    DEFAULT_SCORE_AWARE_SAMPLES
                } else {
                    samples
                };
                Some(BaselineAgent::ScoreAware(Box::new(ScoreAwarePlayer::new(
                    k, seed,
                ))))
            }
            "clean-split-close" => {
                let k = if samples == DEFAULT_SEARCH_SAMPLES {
                    DEFAULT_CLEAN_SPLIT_SAMPLES
                } else {
                    samples
                };
                Some(BaselineAgent::CleanSplitClose(Box::new(
                    CleanSplitCloseWrapper::new(RuleBasedPlayer::new(), k, seed),
                )))
            }
            _ => None,
        }
    }

    pub fn names() -> &'static [&'static str] {
        &[
            "random",
            "greedy",
            "rule-based",
            "search",
            "score-aware",
            "clean-split-close",
        ]
    }
}

impl Player for BaselineAgent {
    fn act(&mut self, obs: &Observation, legal: &[Action]) -> Action {
        match self {
            BaselineAgent::Random(p) => p.act(obs, legal),
            BaselineAgent::Greedy(p) => p.act(obs, legal),
            BaselineAgent::RuleBased(p) => p.act(obs, legal),
            BaselineAgent::Search(p) => p.act(obs, legal),
            BaselineAgent::ScoreAware(p) => p.act(obs, legal),
            BaselineAgent::CleanSplitClose(p) => p.act(obs, legal),
        }
    }

    fn notify(&mut self, event: &Event) {
        match self {
            BaselineAgent::Random(p) => p.notify(event),
            BaselineAgent::Greedy(p) => p.notify(event),
            BaselineAgent::RuleBased(p) => p.notify(event),
            BaselineAgent::Search(p) => p.notify(event),
            BaselineAgent::ScoreAware(p) => p.notify(event),
            BaselineAgent::CleanSplitClose(p) => p.notify(event),
        }
    }
}

/// Aggregated statistics across a batch of paired matches.
#[derive(Debug, Clone, PartialEq)]
pub struct EvalReport {
    pub games: u32,
    pub a_wins: u32,
    pub b_wins: u32,
    pub draws: u32,
    pub mean_a_score: f64,
    pub mean_b_score: f64,
    pub hands_per_match: f64,
    pub duration_ms: u64,
    pub decisions_per_second: f64,
}

impl EvalReport {
    pub fn a_win_rate(&self) -> f64 {
        if self.games == 0 {
            0.0
        } else {
            self.a_wins as f64 / self.games as f64
        }
    }

    /// 95% Wilson score interval on agent A's win rate.
    pub fn a_win_rate_ci(&self) -> (f64, f64) {
        wilson_ci(self.a_wins, self.games, 1.96)
    }
}

/// Fold a list of `PairedResult`s plus the wall-clock duration into an
/// `EvalReport`. Each pair contributes exactly two matches; agent A is
/// seat 0 in the first match and seat 1 in the second.
pub fn report_from_pairs(pairs: &[PairedResult], duration: Duration) -> EvalReport {
    let mut a_wins = 0u32;
    let mut b_wins = 0u32;
    let mut draws = 0u32;
    let mut a_score_sum: i64 = 0;
    let mut b_score_sum: i64 = 0;
    let mut total_hands: u64 = 0;
    let mut total_decisions: u64 = 0;

    for pair in pairs {
        // Match 1: A is seat 0, B is seat 1.
        match pair.a_on_seat_0.winner_seat {
            Some(s) if s == Seat::new(0) => a_wins += 1,
            Some(s) if s == Seat::new(1) => b_wins += 1,
            _ => draws += 1,
        }
        if pair.a_on_seat_0.final_scores.len() >= 2 {
            a_score_sum += pair.a_on_seat_0.final_scores[0] as i64;
            b_score_sum += pair.a_on_seat_0.final_scores[1] as i64;
        }
        total_hands += pair.a_on_seat_0.hands_played as u64;
        total_decisions += pair.a_on_seat_0.decisions;

        // Match 2: A is seat 1, B is seat 0.
        match pair.a_on_seat_1.winner_seat {
            Some(s) if s == Seat::new(1) => a_wins += 1,
            Some(s) if s == Seat::new(0) => b_wins += 1,
            _ => draws += 1,
        }
        if pair.a_on_seat_1.final_scores.len() >= 2 {
            a_score_sum += pair.a_on_seat_1.final_scores[1] as i64;
            b_score_sum += pair.a_on_seat_1.final_scores[0] as i64;
        }
        total_hands += pair.a_on_seat_1.hands_played as u64;
        total_decisions += pair.a_on_seat_1.decisions;
    }

    let games = (pairs.len() as u32).saturating_mul(2);
    let games_f = games as f64;
    let duration_s = duration.as_secs_f64().max(1e-9);

    EvalReport {
        games,
        a_wins,
        b_wins,
        draws,
        mean_a_score: if games > 0 {
            a_score_sum as f64 / games_f
        } else {
            0.0
        },
        mean_b_score: if games > 0 {
            b_score_sum as f64 / games_f
        } else {
            0.0
        },
        hands_per_match: if games > 0 {
            total_hands as f64 / games_f
        } else {
            0.0
        },
        duration_ms: duration.as_millis() as u64,
        decisions_per_second: total_decisions as f64 / duration_s,
    }
}

/// Wilson score interval. Returns `(lo, hi)` clamped to `[0.0, 1.0]`.
/// `wins` is the number of successes out of `n` trials; `z` is the
/// standard-normal critical value (use 1.96 for a 95% interval). On
/// `n == 0` returns the vacuous interval `(0.0, 1.0)` so callers do
/// not have to special-case empty samples.
pub fn wilson_ci(wins: u32, n: u32, z: f64) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let n = n as f64;
    let p = wins as f64 / n;
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denom;
    let margin = (z / denom) * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    (
        (center - margin).clamp(0.0, 1.0),
        (center + margin).clamp(0.0, 1.0),
    )
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
