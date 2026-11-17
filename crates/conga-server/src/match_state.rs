//! Match-level state: scores across hands, dealer rotation, elimination
//! tracking, overall status. This is orchestration, not rules — the
//! per-hand engine (`RuleSet`) stays unaware of match concerns.

use conga_core::{HandResult, MatchStatus, RuleSet, Seat};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchState {
    pub seats_count: u8,
    /// Cumulative points per seat, in seat order. Can go below zero because
    /// of the -10 and -50 closer bonuses.
    pub scores: Vec<i32>,
    /// The seat that deals the next hand. The seat after `dealer` plays
    /// first (rule 16 and C3).
    pub dealer: Seat,
    pub hands_played: u32,
    pub status: MatchStatus,
}

impl MatchState {
    pub fn new(seats_count: u8, starting_dealer: Seat) -> Self {
        assert!(
            (2..=4).contains(&seats_count),
            "seats_count must be in 2..=4, got {seats_count}"
        );
        assert!(
            starting_dealer.index() < seats_count,
            "starting_dealer {} out of range for {} seats",
            starting_dealer.index(),
            seats_count
        );
        MatchState {
            seats_count,
            scores: vec![0; seats_count as usize],
            dealer: starting_dealer,
            hands_played: 0,
            status: MatchStatus::InProgress,
        }
    }

    /// Fold a hand's result into the running scores, bump the hand count
    /// and rotate the dealer by `+1 mod seats_count` (rule 16). Consults
    /// the rule set's `match_status` with the new scores and updates
    /// `self.status`.
    pub fn finish_hand<R: RuleSet>(&mut self, rules: &R, result: &HandResult) {
        assert_eq!(
            result.points.len(),
            self.scores.len(),
            "HandResult has wrong seat count"
        );
        for (score, add) in self.scores.iter_mut().zip(result.points.iter()) {
            *score += *add;
        }
        self.hands_played += 1;
        self.status = rules.match_status(&self.scores);
        self.dealer = self.dealer.next(self.seats_count);
    }

    pub fn is_finished(&self) -> bool {
        !matches!(self.status, MatchStatus::InProgress)
    }
}
