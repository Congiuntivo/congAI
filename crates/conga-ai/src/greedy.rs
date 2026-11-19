//! `GreedyPlayer`: deterministic heuristic agent.
//!
//! Per D27:
//! - Close whenever `can_close` is in the legal set.
//! - Lay a legal meld or add to one when any is offered (laying always
//!   shrinks the hand).
//! - Draw from the discard pile if the top card lowers the solver's
//!   unmatched total by more than its own value (i.e. it actually fits
//!   a meld); otherwise draw from stock.
//! - Discard the highest-rank card from the solver's unmatched set.
//!   Ties broken by card index. If the unmatched set is empty the
//!   player falls back to the highest-rank card among the legal
//!   discards.
//! - Pass only when no other action is legal.
//!
//! No RNG, no internal state across turns: reproducible from the match
//! seed alone.

use conga_core::{
    Action, Card, CloseParams, MeldParams, Observation, best_meld_split, evaluate_hand, standard_40,
};

use crate::player::Player;

#[derive(Debug, Default, Clone, Copy)]
pub struct GreedyPlayer;

impl GreedyPlayer {
    pub fn new() -> Self {
        GreedyPlayer
    }
}

impl Player for GreedyPlayer {
    fn act(&mut self, obs: &Observation, legal: &[Action]) -> Action {
        debug_assert!(!legal.is_empty(), "GreedyPlayer::act with empty legal set");

        if let Some(a) = legal.iter().find(|a| matches!(a, Action::Close(_))) {
            return a.clone();
        }
        if let Some(a) = legal.iter().find(|a| matches!(a, Action::LayMeld(_))) {
            return a.clone();
        }
        if let Some(a) = legal.iter().find(|a| matches!(a, Action::AddToMeld { .. })) {
            return a.clone();
        }

        let has_stock = legal.contains(&Action::DrawStock);
        let has_discard = legal.contains(&Action::DrawDiscard);
        if has_stock || has_discard {
            return pick_draw(obs, has_stock, has_discard);
        }

        let discards: Vec<Card> = legal
            .iter()
            .filter_map(|a| {
                if let Action::Discard(c) = a {
                    Some(*c)
                } else {
                    None
                }
            })
            .collect();
        if !discards.is_empty() {
            return Action::Discard(pick_discard(obs, &discards));
        }

        // Nothing else legal: a showdown-round Pass, most likely.
        legal[0].clone()
    }
}

fn pick_draw(obs: &Observation, has_stock: bool, has_discard: bool) -> Action {
    match (has_stock, has_discard) {
        (true, false) => Action::DrawStock,
        (false, true) => Action::DrawDiscard,
        _ => {
            let Some(top) = obs.top_discard else {
                return Action::DrawStock;
            };
            let before = evaluate_hand(
                obs.own_hand,
                &MeldParams::standard(),
                &CloseParams::standard(),
                standard_40(),
            )
            .unmatched_total;
            let mut with_top = obs.own_hand;
            with_top.insert(top);
            let after = evaluate_hand(
                with_top,
                &MeldParams::standard(),
                &CloseParams::standard(),
                standard_40(),
            )
            .unmatched_total;
            // The top discard costs its own value if it stays unmatched
            // (worst case). It's worth taking only if the new unmatched
            // total is *less* than the before-total + that cost — i.e.
            // the card completed or extended a meld, lifting other cards
            // out of the unmatched set.
            if after < before + top.rank().as_u8() as u32 {
                Action::DrawDiscard
            } else {
                Action::DrawStock
            }
        }
    }
}

fn pick_discard(obs: &Observation, candidates: &[Card]) -> Card {
    let split = best_meld_split(obs.own_hand, &MeldParams::standard());
    let unmatched: Vec<Card> = split.unmatched.iter().collect();
    // Prefer an unmatched card; among those, the highest rank wins and
    // ties break by card index. Fall back to any candidate if the
    // unmatched set is empty.
    let pool: Vec<Card> = if !unmatched.is_empty() {
        unmatched
            .into_iter()
            .filter(|c| candidates.contains(c))
            .collect()
    } else {
        candidates.to_vec()
    };
    let pool = if pool.is_empty() {
        candidates.to_vec()
    } else {
        pool
    };
    *pool
        .iter()
        .max_by(|a, b| {
            a.rank()
                .as_u8()
                .cmp(&b.rank().as_u8())
                .then(b.index().cmp(&a.index()))
        })
        .expect("at least one candidate")
}
