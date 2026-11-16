//! `StandardConga` - Simone's variant as the first rule set.
//!
//! This slice implements the playing phase (draw, discard, close -> showdown
//! transition, empty-stock reshuffle, first-turn rule, rule 6 "can't
//! re-discard a card just taken from the pile"). The showdown body and
//! scoring arrive in slices 6 and 7.
//!
//! Rule interpretation held in this slice: rule 6 ("taking the top discard
//! and discarding the same card is forbidden") applies only to face-up
//! discards. `Close(c)` with the card just taken from the pile is treated as
//! legal. The stop report flags this for Simone's review.

use std::ops::RangeInclusive;

use conga_core::{
    Action, ActionList, Bonus, Card, Chance, CloseParams, DeckSpec, DrawSource, Event, Hand,
    HandEval, HandResult, MatchStatus, MeldParams, Observation, Phase, Rng, RuleSet, RuleSetId,
    Seat, best_meld_split, evaluate_hand, standard_40,
};

use crate::state::State;

const RULESET_NAME: &str = "conga-standard";
const RULESET_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardConga {
    pub meld_params: MeldParams,
    pub close_params: CloseParams,
    pub seats_range: RangeInclusive<u8>,
    pub hand_size: u8,
    /// A seat's match score strictly greater than this value is out.
    pub elimination_score: i32,
}

impl Default for StandardConga {
    fn default() -> Self {
        StandardConga::new()
    }
}

impl StandardConga {
    pub const fn new() -> Self {
        StandardConga {
            meld_params: MeldParams::standard(),
            close_params: CloseParams::standard(),
            seats_range: 2..=4,
            hand_size: 7,
            elimination_score: 100,
        }
    }

    fn reshuffle_discard_into_stock(
        &self,
        state: &mut State,
        chance: &mut dyn Chance,
        events: &mut Vec<Event>,
    ) {
        if state.discard_pile.len() <= 1 {
            // Nothing to reshuffle.
            return;
        }
        // Keep the top of the pile aside, shuffle the rest into the stock.
        let top = state.discard_pile.pop();
        let mut to_shuffle = std::mem::take(&mut state.discard_pile);
        chance.shuffle(&mut to_shuffle);
        events.push(Event::Shuffled(to_shuffle.clone()));
        state.stock = to_shuffle;
        if let Some(t) = top {
            state.discard_pile.push(t);
        }
    }
}

impl RuleSet for StandardConga {
    type State = State;

    fn id(&self) -> RuleSetId {
        RuleSetId {
            name: RULESET_NAME,
            version: RULESET_VERSION,
            // Parameters of StandardConga are currently a known constant; a
            // real hash of the meld and close params moves here if a rule
            // variant with different parameters is added.
            param_hash: 0,
        }
    }

    fn deck(&self) -> DeckSpec {
        standard_40()
    }

    fn seats(&self) -> RangeInclusive<u8> {
        self.seats_range.clone()
    }

    fn new_hand(&self, scores: &[i32], dealer: Seat, chance: &mut dyn Chance) -> State {
        debug_assert!(self.seats_range.contains(&(scores.len() as u8)));
        let seats_count = scores.len() as u8;

        let mut deck: Vec<Card> = self.deck().all_cards().collect();
        chance.shuffle(&mut deck);

        // Round-robin deal starting with the player after the dealer
        // (rule 16). pop() takes from the end of the shuffled deck.
        let first_to_act = dealer.next(seats_count);
        let mut hands = vec![Hand::EMPTY; seats_count as usize];
        for i in 0..(self.hand_size as usize * seats_count as usize) {
            match deck.pop() {
                Some(card) => {
                    let seat_idx = (first_to_act.index() as usize + i) % seats_count as usize;
                    hands[seat_idx].insert(card);
                }
                None => break,
            }
        }

        // Start the discard pile with one card.
        let mut discard_pile = Vec::new();
        if let Some(top) = deck.pop() {
            discard_pile.push(top);
        }

        // The remaining cards are the stock.
        let stock = deck;

        State {
            seats_count,
            hands,
            stock,
            discard_pile,
            scores: scores.to_vec(),
            dealer,
            current_seat: first_to_act,
            phase: Phase::Draw,
            melds_on_table: Vec::new(),
            first_turn_done: vec![false; seats_count as usize],
            must_not_discard: None,
            closer: None,
            closer_hand_at_close: None,
        }
    }

    fn legal_actions(&self, state: &State, out: &mut ActionList) {
        out.clear();
        match state.phase {
            Phase::Draw => {
                out.push(Action::DrawStock);
                // You can always try to reshuffle on DrawStock when stock is
                // empty, so DrawStock is listed even when stock is empty.
                if !state.discard_pile.is_empty() {
                    out.push(Action::DrawDiscard);
                }
            }
            Phase::Discard => {
                let seat_idx = state.current_seat.index() as usize;
                let hand = state.hands[seat_idx];
                let is_first_turn = !state.first_turn_done[seat_idx];

                for c in hand.iter() {
                    if Some(c) == state.must_not_discard {
                        continue;
                    }
                    out.push(Action::Discard(c));
                }

                if !is_first_turn {
                    for c in hand.iter() {
                        let mut residual = hand;
                        residual.remove(c);
                        let split = best_meld_split(residual, &self.meld_params);
                        if self.close_params.can_close(split.unmatched) {
                            out.push(Action::Close(c));
                        }
                    }
                }
            }
            Phase::Showdown => {
                // Slice 6 fills in the showdown legal actions. In slice 5,
                // the test policy never closes, so this branch is unreached.
            }
            Phase::Ended => {
                // No legal actions once the hand has ended.
            }
        }
    }

    fn apply(
        &self,
        state: &mut State,
        action: Action,
        chance: &mut dyn Chance,
        events: &mut Vec<Event>,
    ) {
        match (state.phase, action) {
            (Phase::Draw, Action::DrawStock) => {
                if state.stock.is_empty() {
                    self.reshuffle_discard_into_stock(state, chance, events);
                }
                match state.stock.pop() {
                    Some(card) => {
                        state.hands[state.current_seat.index() as usize].insert(card);
                        events.push(Event::Drew {
                            seat: state.current_seat,
                            source: DrawSource::Stock,
                        });
                        state.must_not_discard = None;
                        state.phase = Phase::Discard;
                    }
                    None => {
                        // Stock was empty and the discard pile had only its
                        // top card: no cards available to draw. Slice 5 does
                        // not yet define this terminal condition; the test
                        // policy with a turn cap never reaches it.
                        unreachable!("stock empty after reshuffle attempt; terminal case deferred");
                    }
                }
            }
            (Phase::Draw, Action::DrawDiscard) => match state.discard_pile.pop() {
                Some(card) => {
                    state.hands[state.current_seat.index() as usize].insert(card);
                    state.must_not_discard = Some(card);
                    events.push(Event::Drew {
                        seat: state.current_seat,
                        source: DrawSource::TopDiscard,
                    });
                    state.phase = Phase::Discard;
                }
                None => {
                    unreachable!("DrawDiscard called with empty pile");
                }
            },
            (Phase::Discard, Action::Discard(c)) => {
                let seat_idx = state.current_seat.index() as usize;
                state.hands[seat_idx].remove(c);
                state.discard_pile.push(c);
                events.push(Event::Discarded {
                    seat: state.current_seat,
                    card: c,
                });
                state.first_turn_done[seat_idx] = true;
                state.must_not_discard = None;
                state.current_seat = state.current_seat.next(state.seats_count);
                state.phase = Phase::Draw;
            }
            (Phase::Discard, Action::Close(c)) => {
                let seat_idx = state.current_seat.index() as usize;
                state.hands[seat_idx].remove(c);
                // Face-down discard: not placed on the discard pile; the
                // engine records it only in the event log.
                events.push(Event::Closed {
                    seat: state.current_seat,
                    card: c,
                });
                state.first_turn_done[seat_idx] = true;
                state.must_not_discard = None;
                state.closer = Some(state.current_seat);
                state.closer_hand_at_close = Some(state.hands[seat_idx]);
                state.phase = Phase::Showdown;
                // current_seat stays on the closer for the first showdown turn
                // (slice 6).
            }
            _ => {
                debug_assert!(false, "unexpected action for phase {:?}", state.phase);
            }
        }
    }

    fn observe(&self, state: &State, seat: Seat) -> Observation {
        let seat_idx = seat.index() as usize;
        Observation {
            own_seat: seat,
            current_seat: state.current_seat,
            phase: state.phase,
            own_hand: state.hands[seat_idx],
            top_discard: state.discard_pile.last().copied(),
            discard_history: state.discard_pile.clone(),
            stock_size: state.stock.len() as u8,
            melds_on_table: state.melds_on_table.clone(),
            scores: state.scores.clone(),
            hand_sizes: state.hands.iter().map(|h| h.len() as u8).collect(),
        }
    }

    fn hand_result(&self, state: &State) -> Option<HandResult> {
        if state.phase != Phase::Ended {
            return None;
        }
        // Slice 7 fills in real scoring with the -10 and -50 bonuses and the
        // per-seat sums. In slice 5, no code path ever sets phase to Ended,
        // so this branch is only reachable in later slices.
        let bonus: Option<Bonus> = None;
        Some(HandResult {
            points: vec![0; state.seats_count as usize],
            closer: state.closer,
            bonus,
        })
    }

    fn match_status(&self, scores: &[i32]) -> MatchStatus {
        let active: Vec<usize> = scores
            .iter()
            .enumerate()
            .filter(|&(_, &s)| s <= self.elimination_score)
            .map(|(i, _)| i)
            .collect();
        match active.len() {
            0 => match scores.iter().enumerate().min_by_key(|&(_, &s)| s) {
                Some((i, _)) => MatchStatus::Winner(Seat::new(i as u8)),
                None => MatchStatus::InProgress,
            },
            1 => MatchStatus::Winner(Seat::new(active[0] as u8)),
            _ => MatchStatus::InProgress,
        }
    }

    fn evaluate_hand(&self, hand: Hand) -> HandEval {
        evaluate_hand(hand, &self.meld_params, &self.close_params, self.deck())
    }

    fn sample_state(&self, _obs: &Observation, _rng: &mut dyn Rng) -> State {
        unimplemented!("StandardConga::sample_state arrives in milestone 4 with the search")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conga_core::{DrawSource, Rank, Suit};
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;

    struct RandomChance(ChaCha8Rng);

    impl Chance for RandomChance {
        fn shuffle(&mut self, deck: &mut Vec<Card>) {
            // Fisher-Yates using the Rng trait. The small bias from
            // `next_u32() % (i+1)` is negligible on a 40-card deck and the
            // shuffle is logged, so replay stays exact.
            for i in (1..deck.len()).rev() {
                let j = (self.0.next_u32() as usize) % (i + 1);
                deck.swap(i, j);
            }
        }
    }

    fn chance(seed: u64) -> RandomChance {
        RandomChance(ChaCha8Rng::seed_from_u64(seed))
    }

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    // --- new_hand ---

    #[test]
    fn deal_2_players() {
        let rules = StandardConga::new();
        let mut rng = chance(1);
        let state = rules.new_hand(&[0, 0], Seat::new(0), &mut rng);
        assert_eq!(state.seats_count, 2);
        assert_eq!(state.hands.len(), 2);
        for h in &state.hands {
            assert_eq!(h.len(), 7);
        }
        assert_eq!(state.stock.len(), 40 - 2 * 7 - 1);
        assert_eq!(state.discard_pile.len(), 1);
        assert_eq!(state.current_seat, Seat::new(1));
        assert_eq!(state.phase, Phase::Draw);
        assert!(state.first_turn_done.iter().all(|&b| !b));
        assert_eq!(state.must_not_discard, None);
        assert_eq!(state.closer, None);
    }

    #[test]
    fn deal_3_and_4_players() {
        let rules = StandardConga::new();
        for seats in [3u8, 4] {
            let mut rng = chance(42 + seats as u64);
            let scores = vec![0i32; seats as usize];
            let state = rules.new_hand(&scores, Seat::new(0), &mut rng);
            assert_eq!(state.seats_count, seats);
            assert_eq!(state.hands.len(), seats as usize);
            for h in &state.hands {
                assert_eq!(h.len(), 7);
            }
            assert_eq!(state.stock.len(), 40 - 7 * seats as usize - 1);
            assert_eq!(state.discard_pile.len(), 1);
            assert_eq!(state.current_seat, Seat::new(1));
        }
    }

    #[test]
    fn deal_accounts_for_all_40_cards() {
        let rules = StandardConga::new();
        let mut rng = chance(7);
        let state = rules.new_hand(&[0, 0, 0], Seat::new(1), &mut rng);
        let mut seen = Hand::EMPTY;
        for h in &state.hands {
            for c in h.iter() {
                assert!(!seen.contains(c));
                seen.insert(c);
            }
        }
        for &c in &state.stock {
            assert!(!seen.contains(c));
            seen.insert(c);
        }
        for &c in &state.discard_pile {
            assert!(!seen.contains(c));
            seen.insert(c);
        }
        assert_eq!(seen.len(), 40);
    }

    // --- legal_actions ---

    #[test]
    fn draw_phase_offers_stock_and_discard() {
        let rules = StandardConga::new();
        let mut rng = chance(0);
        let state = rules.new_hand(&[0, 0], Seat::new(0), &mut rng);
        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        assert!(actions.contains(&Action::DrawStock));
        assert!(actions.contains(&Action::DrawDiscard));
    }

    #[test]
    fn discard_phase_excludes_the_card_just_taken_from_the_pile() {
        let rules = StandardConga::new();
        let mut rng = chance(0);
        let mut state = rules.new_hand(&[0, 0], Seat::new(0), &mut rng);
        let taken = *state.discard_pile.last().unwrap();
        let mut events = Vec::new();
        rules.apply(&mut state, Action::DrawDiscard, &mut rng, &mut events);
        assert_eq!(state.phase, Phase::Discard);
        assert_eq!(state.must_not_discard, Some(taken));

        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        assert!(
            !actions.contains(&Action::Discard(taken)),
            "the just-taken card must not be in legal discards"
        );
    }

    #[test]
    fn first_turn_close_is_forbidden() {
        let rules = StandardConga::new();
        // Build a state where the current seat has a closing hand on their
        // first turn. Easiest: inject by constructing a State directly.
        let hand: Hand = [
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Clubs, 2),
            c(Suit::Diamonds, 2),
            c(Suit::Hearts, 2),
            c(Suit::Spades, 3),
            // an 8th card for the discard phase
            c(Suit::Spades, 4),
        ]
        .as_slice()
        .into();
        let state = State {
            seats_count: 2,
            hands: vec![hand, Hand::EMPTY],
            stock: Vec::new(),
            discard_pile: vec![c(Suit::Clubs, 10)],
            scores: vec![0, 0],
            dealer: Seat::new(1),
            current_seat: Seat::new(0),
            phase: Phase::Discard,
            melds_on_table: Vec::new(),
            first_turn_done: vec![false, false],
            must_not_discard: None,
            closer: None,
            closer_hand_at_close: None,
        };
        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        assert!(
            actions.iter().all(|a| !matches!(a, Action::Close(_))),
            "no Close action is legal on the seat's first turn"
        );
    }

    #[test]
    fn second_turn_close_is_legal_when_hand_allows() {
        let rules = StandardConga::new();
        // Same closing hand, but mark first_turn_done.
        let hand: Hand = [
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Clubs, 2),
            c(Suit::Diamonds, 2),
            c(Suit::Hearts, 2),
            c(Suit::Spades, 3),
            c(Suit::Spades, 4),
        ]
        .as_slice()
        .into();
        let state = State {
            seats_count: 2,
            hands: vec![hand, Hand::EMPTY],
            stock: Vec::new(),
            discard_pile: vec![c(Suit::Clubs, 10)],
            scores: vec![0, 0],
            dealer: Seat::new(1),
            current_seat: Seat::new(0),
            phase: Phase::Discard,
            melds_on_table: Vec::new(),
            first_turn_done: vec![true, false],
            must_not_discard: None,
            closer: None,
            closer_hand_at_close: None,
        };
        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        // Discarding the 4S leaves a closing hand (1 unmatched of value 3).
        assert!(actions.contains(&Action::Close(c(Suit::Spades, 4))));
    }

    // --- apply ---

    #[test]
    fn draw_stock_moves_a_card_into_hand_and_advances_phase() {
        let rules = StandardConga::new();
        let mut rng = chance(3);
        let mut state = rules.new_hand(&[0, 0], Seat::new(0), &mut rng);
        let pre_hand_size = state.hands[1].len();
        let pre_stock = state.stock.len();
        let mut events = Vec::new();
        rules.apply(&mut state, Action::DrawStock, &mut rng, &mut events);
        assert_eq!(state.hands[1].len(), pre_hand_size + 1);
        assert_eq!(state.stock.len(), pre_stock - 1);
        assert_eq!(state.phase, Phase::Discard);
        assert!(matches!(
            events.last(),
            Some(Event::Drew {
                source: DrawSource::Stock,
                ..
            })
        ));
    }

    #[test]
    fn discard_moves_card_from_hand_to_pile_and_advances_seat() {
        let rules = StandardConga::new();
        let mut rng = chance(5);
        let mut state = rules.new_hand(&[0, 0], Seat::new(0), &mut rng);
        let mut events = Vec::new();
        rules.apply(&mut state, Action::DrawStock, &mut rng, &mut events);
        // Pick any card in hand.
        let seat_idx = state.current_seat.index() as usize;
        let to_discard = state.hands[seat_idx].iter().next().unwrap();
        let pre_hand = state.hands[seat_idx].len();
        let pre_pile = state.discard_pile.len();
        let acting = state.current_seat;
        rules.apply(
            &mut state,
            Action::Discard(to_discard),
            &mut rng,
            &mut events,
        );
        assert_eq!(state.hands[seat_idx].len(), pre_hand - 1);
        assert_eq!(state.discard_pile.len(), pre_pile + 1);
        assert_eq!(state.phase, Phase::Draw);
        assert_eq!(state.current_seat, acting.next(2));
        assert!(state.first_turn_done[seat_idx]);
    }

    // --- reshuffle ---

    #[test]
    fn empty_stock_reshuffles_discard_except_top() {
        let rules = StandardConga::new();
        let mut rng = chance(11);
        let mut state = rules.new_hand(&[0, 0], Seat::new(0), &mut rng);
        // Drain the stock by hand and move cards to the discard pile.
        // Build a state with stock=[] and pile with 10 cards.
        let mut synthetic_pile: Vec<Card> = std::mem::take(&mut state.stock);
        synthetic_pile.push(state.discard_pile.pop().unwrap());
        state.discard_pile = synthetic_pile;
        assert!(state.stock.is_empty());
        let pile_top_before = *state.discard_pile.last().unwrap();
        let pile_len_before = state.discard_pile.len();

        let mut events = Vec::new();
        rules.apply(&mut state, Action::DrawStock, &mut rng, &mut events);
        // After reshuffle + draw: stock has pile_len_before - 2 cards (minus
        // one for reshuffle-preserved top, minus one for the draw).
        assert_eq!(state.stock.len(), pile_len_before - 2);
        assert_eq!(state.discard_pile.len(), 1);
        assert_eq!(*state.discard_pile.last().unwrap(), pile_top_before);
        assert!(events.iter().any(|e| matches!(e, Event::Shuffled(_))));
    }

    // --- invariants over random games ---

    /// Pick a random legal action, skipping any `Close(_)` so the test stays
    /// in the draw / discard phases of slice 5.
    fn pick_random_no_close(actions: &[Action], rng: &mut ChaCha8Rng) -> Option<Action> {
        let legal: Vec<&Action> = actions
            .iter()
            .filter(|a| !matches!(a, Action::Close(_)))
            .collect();
        if legal.is_empty() {
            return None;
        }
        let idx = (rng.next_u32() as usize) % legal.len();
        Some(legal[idx].clone())
    }

    fn assert_cards_accounted_for(rules: &StandardConga, state: &State) {
        let mut seen = Hand::EMPTY;
        let mut total: u32 = 0;
        for h in &state.hands {
            for c in h.iter() {
                assert!(!seen.contains(c), "duplicate card {c:?}");
                seen.insert(c);
                total += 1;
            }
        }
        for &c in state.stock.iter().chain(state.discard_pile.iter()) {
            assert!(!seen.contains(c), "duplicate card {c:?}");
            seen.insert(c);
            total += 1;
        }
        assert_eq!(total, rules.deck().cards as u32);
        assert_eq!(seen.len(), rules.deck().cards as u32);
    }

    fn assert_observation_hides_opponents(rules: &StandardConga, state: &State) {
        for seat_idx in 0..state.seats_count {
            let seat = Seat::new(seat_idx);
            let obs = rules.observe(state, seat);
            assert_eq!(obs.own_hand, state.hands[seat_idx as usize]);
            // The observation's public cards must not include any opponent's
            // hand card.
            let mut public = Hand::EMPTY;
            for &c in obs.discard_history.iter() {
                public.insert(c);
            }
            // The own hand is of course "visible" to itself, so skip that
            // seat.
            for other_idx in 0..state.seats_count {
                if other_idx == seat_idx {
                    continue;
                }
                let other_hand = state.hands[other_idx as usize];
                for c in other_hand.iter() {
                    assert!(
                        !public.contains(c),
                        "opponent card {c:?} leaked into seat {seat_idx}'s observation",
                    );
                }
            }
        }
    }

    #[test]
    fn invariants_hold_over_random_games() {
        let rules = StandardConga::new();
        for &seats in &[2u8, 3, 4] {
            for seed in 0..20u64 {
                let mut chance_rng = chance(seed);
                let mut policy_rng = ChaCha8Rng::seed_from_u64(seed ^ 0xA5A5A5A5A5A5A5A5);
                let scores = vec![0i32; seats as usize];
                let mut state = rules.new_hand(&scores, Seat::new(0), &mut chance_rng);
                let mut events: Vec<Event> = Vec::new();
                let mut actions = ActionList::new();

                for _turn in 0..500u32 {
                    assert_cards_accounted_for(&rules, &state);
                    assert_observation_hides_opponents(&rules, &state);
                    if state.phase == Phase::Showdown || state.phase == Phase::Ended {
                        break;
                    }
                    rules.legal_actions(&state, &mut actions);
                    let chosen = match pick_random_no_close(&actions, &mut policy_rng) {
                        Some(a) => a,
                        None => break,
                    };
                    rules.apply(&mut state, chosen, &mut chance_rng, &mut events);
                }
            }
        }
    }
}
