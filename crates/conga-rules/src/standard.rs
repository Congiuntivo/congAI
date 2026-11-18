//! `StandardConga` - Simone's variant as the first rule set.
//!
//! This covers the full hand loop except the final scoring, which lands in
//! slice 7: draw, discard, close -> showdown transition, empty-stock
//! reshuffle, first-turn rule, rule 6 (face-up discard ban), the showdown
//! placement/addition/pass actions and the closer's detail-(c) reducing
//! constraint.
//!
//! Rule interpretations encoded here (documented in docs/design.md under
//! "Rule clarifications"):
//!
//! C1: Rule 6 applies only to face-up discard. `Close(c)` with the card just
//! taken from the discard pile is legal.
//!
//! C2: Detail (c) is read strictly - the closer's Pass is legal only when
//! their remaining hand is itself a closing residual (<= 2 cards, sum <= 5).
//!
//! C3: The starter (seat after the dealer) is dealt one extra card. The
//! discard pile begins empty and the starter's first action is a Discard,
//! not a Draw. The first-turn close ban still applies.

use std::ops::RangeInclusive;

use conga_core::{
    Action, ActionList, Bonus, Card, Chance, CloseParams, DeckSpec, DrawSource, Event, Hand,
    HandEval, HandResult, MatchStatus, Meld, MeldKind, MeldParams, Observation, Phase, Rank, Rng,
    RuleSet, RuleSetId, Seat, Suit, best_meld_split, evaluate_hand, standard_40,
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

    /// Every valid meld that can be formed from `hand`, under this rule set's
    /// meld parameters. Used to enumerate `LayMeld` legal actions in the
    /// showdown.
    fn hand_melds(&self, hand: Hand) -> Vec<Meld> {
        let mut out = Vec::new();

        // Sets, grouped by rank.
        for rank_val in Rank::MIN..=Rank::MAX {
            let rank = match Rank::new(rank_val) {
                Some(r) => r,
                None => continue,
            };
            let mut same_rank: Vec<Card> = Vec::new();
            for suit in Suit::ALL {
                let c = Card::new(suit, rank);
                if hand.contains(c) {
                    same_rank.push(c);
                }
            }
            let n = same_rank.len();
            // k = 3
            if self.meld_params.min_set <= 3 && 3 <= self.meld_params.max_set {
                for i in 0..n {
                    for j in (i + 1)..n {
                        for h in (j + 1)..n {
                            let cards =
                                Hand::from([same_rank[i], same_rank[j], same_rank[h]].as_slice());
                            out.push(Meld {
                                kind: MeldKind::Set,
                                cards,
                            });
                        }
                    }
                }
            }
            // k = 4: only one way (all four).
            if self.meld_params.min_set <= 4 && 4 <= self.meld_params.max_set && n == 4 {
                out.push(Meld {
                    kind: MeldKind::Set,
                    cards: Hand::from(same_rank.as_slice()),
                });
            }
        }

        // Runs, grouped by suit, enumerate every sub-run of length >= min_run.
        for suit in Suit::ALL {
            let suit_cards: Vec<Card> = (Rank::MIN..=Rank::MAX)
                .filter_map(|r| {
                    let rank = Rank::new(r)?;
                    let c = Card::new(suit, rank);
                    if hand.contains(c) { Some(c) } else { None }
                })
                .collect();
            let mut i = 0;
            while i < suit_cards.len() {
                let mut j = i;
                while j + 1 < suit_cards.len()
                    && suit_cards[j + 1].rank().as_u8() == suit_cards[j].rank().as_u8() + 1
                {
                    j += 1;
                }
                // The slice suit_cards[i..=j] is a maximal consecutive block.
                let min_run = self.meld_params.min_run as usize;
                for start in i..=j {
                    let min_end = start + min_run - 1;
                    for end in min_end..=j {
                        let run_slice = &suit_cards[start..=end];
                        out.push(Meld {
                            kind: MeldKind::Run,
                            cards: Hand::from(run_slice),
                        });
                    }
                }
                i = j + 1;
            }
        }

        out
    }

    /// Whether `card` can extend the tabled meld.
    fn can_add_to_meld(&self, tabled: &Meld, card: Card) -> bool {
        if tabled.cards.contains(card) {
            return false;
        }
        match tabled.kind {
            MeldKind::Set => {
                if tabled.cards.len() >= self.meld_params.max_set as u32 {
                    return false;
                }
                // All cards in a set share a rank; grab the first.
                let Some(first) = tabled.cards.iter().next() else {
                    return false;
                };
                card.rank() == first.rank()
            }
            MeldKind::Run => {
                let Some(first) = tabled.cards.iter().next() else {
                    return false;
                };
                if card.suit() != first.suit() {
                    return false;
                }
                let mut min_r = u8::MAX;
                let mut max_r = 0u8;
                for c in tabled.cards.iter() {
                    let r = c.rank().as_u8();
                    min_r = min_r.min(r);
                    max_r = max_r.max(r);
                }
                let cr = card.rank().as_u8();
                (min_r > Rank::MIN && cr + 1 == min_r) || (max_r < Rank::MAX && cr == max_r + 1)
            }
        }
    }

    /// The Conga bonus trigger: a 7-card run in one suit. Checked via
    /// bitmask: in the standard-40 encoding a 7-card run occupies 7
    /// consecutive bits within one of the four 10-bit suit groups.
    fn is_conga(hand: Hand) -> bool {
        if hand.len() != 7 {
            return false;
        }
        let bits = hand.bits();
        for suit_idx in 0..4u8 {
            let base = suit_idx * (Rank::MAX);
            // Starting ranks 1..=4 yield 7-card runs ending at 7..=10.
            for start_offset in 0..=(Rank::MAX - 7) {
                let pattern = 0x7Fu64 << (base + start_offset);
                if bits == pattern {
                    return true;
                }
            }
        }
        false
    }

    fn detect_close_bonus(&self, closer_hand_at_close: Hand) -> Option<Bonus> {
        if Self::is_conga(closer_hand_at_close) {
            return Some(Bonus::MinusFifty);
        }
        let split = best_meld_split(closer_hand_at_close, &self.meld_params);
        if split.unmatched.is_empty() {
            return Some(Bonus::MinusTen);
        }
        None
    }

    /// The hand that would remain after applying a showdown placement action.
    fn hand_after_showdown_placement(&self, hand: Hand, action: &Action) -> Hand {
        let mut h = hand;
        match action {
            Action::LayMeld(m) => {
                for c in m.cards.iter() {
                    h.remove(c);
                }
            }
            Action::AddToMeld { card, .. } => {
                h.remove(*card);
            }
            _ => {}
        }
        h
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
            name: std::borrow::Cow::Borrowed(RULESET_NAME),
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

        // The starter is dealt one extra card. Instead of turning the top
        // of the stock face-up as the discard, the starter's first action
        // is a Discard (no draw). The discard pile begins empty and only
        // fills once the starter discards. See rule clarification C3 in
        // docs/design.md.
        if let Some(extra) = deck.pop() {
            hands[first_to_act.index() as usize].insert(extra);
        }

        let stock = deck;

        State {
            seats_count,
            hands,
            stock,
            discard_pile: Vec::new(),
            scores: scores.to_vec(),
            dealer,
            current_seat: first_to_act,
            phase: Phase::Discard,
            melds_on_table: Vec::new(),
            first_turn_done: vec![false; seats_count as usize],
            must_not_discard: None,
            closer: None,
            closer_hand_at_close: None,
            closer_has_opened: false,
            showdown_consecutive_passes: 0,
            face_down_close: None,
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
                let seat = state.current_seat;
                let seat_idx = seat.index() as usize;
                let hand = state.hands[seat_idx];
                let is_closer = state.closer == Some(seat);
                let is_first_closer_turn = is_closer && !state.closer_has_opened;

                let mut placements: Vec<Action> = Vec::new();
                for m in self.hand_melds(hand) {
                    placements.push(Action::LayMeld(m));
                }
                for (idx, tabled) in state.melds_on_table.iter().enumerate() {
                    for c in hand.iter() {
                        if self.can_add_to_meld(tabled, c) {
                            placements.push(Action::AddToMeld {
                                card: c,
                                meld_index: idx as u8,
                            });
                        }
                    }
                }

                if is_first_closer_turn {
                    // Only placement moves that preserve a path to a closing
                    // residual are legal, per detail (c).
                    for action in &placements {
                        let new_hand = self.hand_after_showdown_placement(hand, action);
                        let split = best_meld_split(new_hand, &self.meld_params);
                        if self.close_params.can_close(split.unmatched) {
                            out.push(action.clone());
                        }
                    }
                    // Pass is legal only when the closer has physically laid
                    // enough melds that their hand IS the unmatched set of a
                    // closing split. In standard Conga that means hand.len()
                    // <= 2 (since min meld size is 3) with sum <= 5.
                    if self.close_params.can_close(hand) {
                        out.push(Action::Pass);
                    }
                } else {
                    for action in &placements {
                        out.push(action.clone());
                    }
                    out.push(Action::Pass);
                }
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
                // Face-down discard: not placed on the discard pile. Stored
                // in `face_down_close` so the card stays accounted for and
                // scoring in slice 7 can reveal it in the final event log.
                state.face_down_close = Some(c);
                events.push(Event::Closed {
                    seat: state.current_seat,
                    card: c,
                });
                state.first_turn_done[seat_idx] = true;
                state.must_not_discard = None;
                state.closer = Some(state.current_seat);
                state.closer_hand_at_close = Some(state.hands[seat_idx]);
                state.phase = Phase::Showdown;
                state.closer_has_opened = false;
                state.showdown_consecutive_passes = 0;
                // current_seat stays on the closer for the first showdown turn.
            }
            (Phase::Showdown, Action::LayMeld(m)) => {
                let seat_idx = state.current_seat.index() as usize;
                for c in m.cards.iter() {
                    state.hands[seat_idx].remove(c);
                }
                state.melds_on_table.push(m.clone());
                events.push(Event::LaidMeld {
                    seat: state.current_seat,
                    meld: m,
                });
                state.showdown_consecutive_passes = 0;
            }
            (Phase::Showdown, Action::AddToMeld { card, meld_index }) => {
                let seat_idx = state.current_seat.index() as usize;
                state.hands[seat_idx].remove(card);
                let target = &mut state.melds_on_table[meld_index as usize];
                target.cards.insert(card);
                events.push(Event::AddedToMeld {
                    seat: state.current_seat,
                    card,
                    meld_index,
                });
                state.showdown_consecutive_passes = 0;
            }
            (Phase::Showdown, Action::Pass) => {
                let seat = state.current_seat;
                events.push(Event::Passed { seat });
                if state.closer == Some(seat) {
                    state.closer_has_opened = true;
                }
                state.showdown_consecutive_passes =
                    state.showdown_consecutive_passes.saturating_add(1);
                if state.showdown_consecutive_passes >= state.seats_count {
                    state.phase = Phase::Ended;
                } else {
                    state.current_seat = seat.next(state.seats_count);
                }
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
        // Each seat scores the value of the cards still in its hand. Cards
        // on the table cost nothing. Rule 8 bonuses apply only to the closer
        // and are checked against the closer's hand at the moment of close.
        let mut points: Vec<i32> = (0..state.seats_count)
            .map(|seat_idx| {
                let hand = state.hands[seat_idx as usize];
                hand.iter().map(|c| c.rank().point_value() as i32).sum()
            })
            .collect();

        let bonus = match (state.closer, state.closer_hand_at_close) {
            (Some(closer_seat), Some(closer_hand)) => {
                let b = self.detect_close_bonus(closer_hand);
                if let Some(kind) = b {
                    let closer_idx = closer_seat.index() as usize;
                    let delta: i32 = match kind {
                        Bonus::MinusTen => -10,
                        Bonus::MinusFifty => -50,
                    };
                    points[closer_idx] += delta;
                }
                b
            }
            _ => None,
        };

        Some(HandResult {
            points,
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
    use conga_core::{DrawSource, GameLog, RandomChance, Rank, Suit, replay};
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;

    fn chance(seed: u64) -> RandomChance<ChaCha8Rng> {
        RandomChance::new(ChaCha8Rng::seed_from_u64(seed))
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
        // Starter (seat 1) gets 8 cards per clarification C3; the other
        // seat gets 7. The discard pile begins empty.
        assert_eq!(state.hands[1].len(), 8);
        assert_eq!(state.hands[0].len(), 7);
        assert_eq!(state.stock.len(), 40 - 7 - 8);
        assert_eq!(state.discard_pile.len(), 0);
        assert_eq!(state.current_seat, Seat::new(1));
        assert_eq!(state.phase, Phase::Discard);
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
            for (s, hand) in state.hands.iter().enumerate() {
                let expected = if s == 1 { 8 } else { 7 };
                assert_eq!(hand.len() as usize, expected);
            }
            let total_dealt = 7 * seats as usize + 1;
            assert_eq!(state.stock.len(), 40 - total_dealt);
            assert_eq!(state.discard_pile.len(), 0);
            assert_eq!(state.current_seat, Seat::new(1));
            assert_eq!(state.phase, Phase::Discard);
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
    fn starter_starts_in_discard_phase_with_8_cards() {
        let rules = StandardConga::new();
        let mut rng = chance(0);
        let state = rules.new_hand(&[0, 0], Seat::new(0), &mut rng);
        assert_eq!(state.phase, Phase::Discard);
        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        // 8 cards, all can be discarded, and no Close on the first turn.
        assert_eq!(
            actions
                .iter()
                .filter(|a| matches!(a, Action::Discard(_)))
                .count(),
            8
        );
        assert!(
            actions.iter().all(|a| !matches!(a, Action::Close(_))),
            "starter cannot close on their first turn"
        );
    }

    #[test]
    fn draw_phase_offers_stock_and_discard_after_starter_discards() {
        let rules = StandardConga::new();
        let mut rng = chance(0);
        let mut state = rules.new_hand(&[0, 0], Seat::new(0), &mut rng);
        let first_card = state.hands[1].iter().next().unwrap();
        let mut events = Vec::new();
        rules.apply(
            &mut state,
            Action::Discard(first_card),
            &mut rng,
            &mut events,
        );
        assert_eq!(state.phase, Phase::Draw);
        assert_eq!(state.current_seat, Seat::new(0));
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
        // Starter discards one card so the pile is non-empty.
        let starter_card = state.hands[1].iter().next().unwrap();
        let mut events = Vec::new();
        rules.apply(
            &mut state,
            Action::Discard(starter_card),
            &mut rng,
            &mut events,
        );
        // Seat 0 is now in Draw phase. Take from the pile.
        let taken = *state.discard_pile.last().unwrap();
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
            closer_has_opened: false,
            showdown_consecutive_passes: 0,
            face_down_close: None,
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
            closer_has_opened: false,
            showdown_consecutive_passes: 0,
            face_down_close: None,
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
        // Move past the starter's forced discard to reach a Draw phase.
        let starter_card = state.hands[1].iter().next().unwrap();
        let mut events = Vec::new();
        rules.apply(
            &mut state,
            Action::Discard(starter_card),
            &mut rng,
            &mut events,
        );
        assert_eq!(state.phase, Phase::Draw);
        assert_eq!(state.current_seat, Seat::new(0));
        let pre_hand_size = state.hands[0].len();
        let pre_stock = state.stock.len();
        rules.apply(&mut state, Action::DrawStock, &mut rng, &mut events);
        assert_eq!(state.hands[0].len(), pre_hand_size + 1);
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
        // Starter (seat 1) is already in Discard phase with 8 cards per C3.
        let seat_idx = state.current_seat.index() as usize;
        let to_discard = state.hands[seat_idx].iter().next().unwrap();
        let pre_hand = state.hands[seat_idx].len();
        let pre_pile = state.discard_pile.len();
        let acting = state.current_seat;
        let mut events = Vec::new();
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
        // Move past the starter's forced discard so the pile is non-empty
        // and the next seat is in Draw phase.
        let starter_card = state.hands[1].iter().next().unwrap();
        let mut events = Vec::new();
        rules.apply(
            &mut state,
            Action::Discard(starter_card),
            &mut rng,
            &mut events,
        );
        // Now stock has 25 cards, pile has 1, phase = Draw for seat 0.
        // Synthesise an empty-stock state: move stock cards under the
        // current pile top.
        let pile_top = state.discard_pile.pop().unwrap();
        let mut synthetic_pile: Vec<Card> = std::mem::take(&mut state.stock);
        synthetic_pile.push(pile_top);
        state.discard_pile = synthetic_pile;
        assert!(state.stock.is_empty());
        let pile_top_before = *state.discard_pile.last().unwrap();
        let pile_len_before = state.discard_pile.len();

        rules.apply(&mut state, Action::DrawStock, &mut rng, &mut events);
        // After reshuffle + draw: stock has pile_len_before - 2 cards (minus
        // one for reshuffle-preserved top, minus one for the draw).
        assert_eq!(state.stock.len(), pile_len_before - 2);
        assert_eq!(state.discard_pile.len(), 1);
        assert_eq!(*state.discard_pile.last().unwrap(), pile_top_before);
        assert!(events.iter().any(|e| matches!(e, Event::Shuffled(_))));
    }

    // --- invariants over random games ---

    fn pick_random(actions: &[Action], rng: &mut ChaCha8Rng) -> Option<Action> {
        if actions.is_empty() {
            return None;
        }
        let idx = (rng.next_u32() as usize) % actions.len();
        Some(actions[idx].clone())
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
        for m in &state.melds_on_table {
            for c in m.cards.iter() {
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
        if let Some(c) = state.face_down_close {
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

                for _turn in 0..1000u32 {
                    assert_cards_accounted_for(&rules, &state);
                    assert_observation_hides_opponents(&rules, &state);
                    if state.phase == Phase::Ended {
                        break;
                    }
                    rules.legal_actions(&state, &mut actions);
                    // Invariant 3: while the hand runs there is always a
                    // legal action.
                    assert!(
                        !actions.is_empty(),
                        "no legal action in phase {:?} (seats={seats} seed={seed})",
                        state.phase
                    );
                    let chosen = match pick_random(&actions, &mut policy_rng) {
                        Some(a) => a,
                        None => break,
                    };
                    rules.apply(&mut state, chosen, &mut chance_rng, &mut events);
                }
            }
        }
    }

    // --- replay ---

    /// Play one random hand with `RandomChance`, log the shuffles and
    /// actions, then replay. Return the live states, the replayed states
    /// and the log so callers can compare.
    fn play_and_log(
        rules: &StandardConga,
        seats: u8,
        seed: u64,
        max_turns: u32,
    ) -> (Vec<State>, GameLog) {
        let mut chance = self::chance(seed);
        let mut policy_rng = ChaCha8Rng::seed_from_u64(seed ^ 0xDEADBEEFDEADBEEF);
        let starting_scores = vec![0i32; seats as usize];
        let dealer = Seat::new(0);
        let mut state = rules.new_hand(&starting_scores, dealer, &mut chance);
        let mut live_states = vec![state.clone()];
        let mut recorded_actions: Vec<Action> = Vec::new();
        let mut events: Vec<Event> = Vec::new();
        let mut actions = ActionList::new();
        for _turn in 0..max_turns {
            if state.phase == Phase::Ended {
                break;
            }
            rules.legal_actions(&state, &mut actions);
            let chosen = match pick_random(&actions, &mut policy_rng) {
                Some(a) => a,
                None => break,
            };
            recorded_actions.push(chosen.clone());
            rules.apply(&mut state, chosen, &mut chance, &mut events);
            live_states.push(state.clone());
        }
        let log = GameLog {
            ruleset_id: rules.id(),
            seats,
            dealer,
            starting_scores,
            actions: recorded_actions,
            shuffles: chance.into_shuffles(),
        };
        (live_states, log)
    }

    #[test]
    fn logged_hand_replays_to_identical_states() {
        let rules = StandardConga::new();
        // Hand-written sanity: one seed, step through and confirm every
        // intermediate state matches.
        let (live, log) = play_and_log(&rules, 2, 123, 300);
        let replayed = replay(&rules, &log);
        assert_eq!(live.len(), replayed.states.len());
        for (i, (a, b)) in live.iter().zip(replayed.states.iter()).enumerate() {
            assert_eq!(a, b, "state divergence at step {i}");
        }
    }

    #[test]
    fn replay_matches_live_over_many_games() {
        // The milestone exit condition: play N random games, log each, and
        // assert that the replayed state trajectory is byte-identical. This
        // activates conformance invariant 4.
        let rules = StandardConga::new();
        for &seats in &[2u8, 3, 4] {
            for seed in 0..100u64 {
                let (live, log) = play_and_log(&rules, seats, seed, 500);
                let replayed = replay(&rules, &log);
                assert_eq!(
                    live.len(),
                    replayed.states.len(),
                    "trajectory length differs for seats={seats} seed={seed}"
                );
                for (i, (a, b)) in live.iter().zip(replayed.states.iter()).enumerate() {
                    assert_eq!(
                        a, b,
                        "state divergence at step {i} for seats={seats} seed={seed}"
                    );
                }
            }
        }
    }

    // --- showdown ---

    /// Drive a specific closing hand into the showdown and return the state
    /// and the closer's Seat. The hand is:
    ///   seat 0: 1C 1D 1H 2C 2D 2H 3S 3D (8 cards after a stock draw).
    /// Closing by discarding 3D leaves 1C 1D 1H 2C 2D 2H 3S (sets of 1s and
    /// 2s plus unmatched 3S, sum 3, closes).
    fn setup_closed_hand() -> (StandardConga, State, Vec<Event>) {
        let rules = StandardConga::new();
        let closing_hand: Hand = [
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Clubs, 2),
            c(Suit::Diamonds, 2),
            c(Suit::Hearts, 2),
            c(Suit::Spades, 3),
            c(Suit::Diamonds, 3),
        ]
        .as_slice()
        .into();
        let opp_hand: Hand = [
            c(Suit::Spades, 5),
            c(Suit::Spades, 6),
            c(Suit::Spades, 7),
            c(Suit::Clubs, 10),
            c(Suit::Diamonds, 10),
            c(Suit::Hearts, 10),
            c(Suit::Spades, 4),
        ]
        .as_slice()
        .into();
        let state = State {
            seats_count: 2,
            hands: vec![closing_hand, opp_hand],
            stock: Vec::new(),
            discard_pile: vec![c(Suit::Clubs, 9)],
            scores: vec![0, 0],
            dealer: Seat::new(1),
            current_seat: Seat::new(0),
            phase: Phase::Discard,
            melds_on_table: Vec::new(),
            first_turn_done: vec![true, true],
            must_not_discard: None,
            closer: None,
            closer_hand_at_close: None,
            closer_has_opened: false,
            showdown_consecutive_passes: 0,
            face_down_close: None,
        };

        let mut events = Vec::new();
        let mut rules_state = state;
        // Close by placing 3D face-down.
        rules.apply(
            &mut rules_state,
            Action::Close(c(Suit::Diamonds, 3)),
            &mut chance(0),
            &mut events,
        );
        (rules, rules_state, events)
    }

    #[test]
    fn close_action_transitions_to_showdown() {
        let (_, state, events) = setup_closed_hand();
        assert_eq!(state.phase, Phase::Showdown);
        assert_eq!(state.closer, Some(Seat::new(0)));
        assert!(state.closer_hand_at_close.is_some());
        assert!(!state.closer_has_opened);
        assert_eq!(state.showdown_consecutive_passes, 0);
        assert!(events.iter().any(|e| matches!(e, Event::Closed { .. })));
    }

    #[test]
    fn closer_first_turn_cannot_pass_until_hand_reduces_to_closing() {
        let (rules, state, _) = setup_closed_hand();
        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        // The closer has a 7-card hand; they cannot pass yet.
        assert!(
            !actions.contains(&Action::Pass),
            "closer should not be able to pass before reducing"
        );
        // But they should have LayMeld options that make progress.
        assert!(
            actions.iter().any(|a| matches!(a, Action::LayMeld(_))),
            "closer should have at least one LayMeld option"
        );
    }

    #[test]
    fn closer_first_turn_rejects_meld_that_breaks_closability() {
        let (rules, state, _) = setup_closed_hand();
        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        // Hand: 1C 1D 1H 2C 2D 2H 3S. Laying a run 1C-2C-3C? No, no 3C.
        // The 'bad' move to break closability is non-existent in this hand
        // because every valid meld (set of 1s, set of 2s) leaves a residual
        // that still closes. Instead: check that every LayMeld action in
        // legal_actions does keep closability.
        for action in &actions {
            if let Action::LayMeld(m) = action {
                let mut remaining = state.hands[0];
                for cc in m.cards.iter() {
                    remaining.remove(cc);
                }
                let split = best_meld_split(remaining, &rules.meld_params);
                assert!(
                    rules.close_params.can_close(split.unmatched),
                    "LayMeld {:?} leaves a non-closing residual",
                    m.cards.bits()
                );
            }
        }
    }

    #[test]
    fn closer_can_pass_once_hand_reduces_to_closing_residual() {
        let (rules, mut state, mut events) = setup_closed_hand();
        // Lay the 1s set.
        let m1 = Meld {
            kind: MeldKind::Set,
            cards: Hand::from(
                [c(Suit::Clubs, 1), c(Suit::Diamonds, 1), c(Suit::Hearts, 1)].as_slice(),
            ),
        };
        let mut rng = chance(0);
        rules.apply(&mut state, Action::LayMeld(m1), &mut rng, &mut events);
        // Lay the 2s set.
        let m2 = Meld {
            kind: MeldKind::Set,
            cards: Hand::from(
                [c(Suit::Clubs, 2), c(Suit::Diamonds, 2), c(Suit::Hearts, 2)].as_slice(),
            ),
        };
        rules.apply(&mut state, Action::LayMeld(m2), &mut rng, &mut events);
        // Now the closer's hand is just 3S. Pass should be legal.
        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        assert!(
            actions.contains(&Action::Pass),
            "closer should be able to pass with a closing residual (3S alone)"
        );
    }

    #[test]
    fn showdown_ends_after_a_full_round_of_passes() {
        let (rules, mut state, mut events) = setup_closed_hand();
        let mut rng = chance(0);
        // Closer lays both sets, reaching a closing residual.
        for cards in [
            [c(Suit::Clubs, 1), c(Suit::Diamonds, 1), c(Suit::Hearts, 1)],
            [c(Suit::Clubs, 2), c(Suit::Diamonds, 2), c(Suit::Hearts, 2)],
        ] {
            let m = Meld {
                kind: MeldKind::Set,
                cards: Hand::from(cards.as_slice()),
            };
            rules.apply(&mut state, Action::LayMeld(m), &mut rng, &mut events);
        }
        // Closer passes, then non-closer passes. Hand ends.
        rules.apply(&mut state, Action::Pass, &mut rng, &mut events);
        assert_eq!(state.phase, Phase::Showdown);
        assert!(state.closer_has_opened);
        rules.apply(&mut state, Action::Pass, &mut rng, &mut events);
        assert_eq!(state.phase, Phase::Ended);
    }

    #[test]
    fn non_closer_may_keep_meld_in_hand() {
        let (rules, mut state, mut events) = setup_closed_hand();
        let mut rng = chance(0);
        // Closer lays both sets and passes.
        for cards in [
            [c(Suit::Clubs, 1), c(Suit::Diamonds, 1), c(Suit::Hearts, 1)],
            [c(Suit::Clubs, 2), c(Suit::Diamonds, 2), c(Suit::Hearts, 2)],
        ] {
            let m = Meld {
                kind: MeldKind::Set,
                cards: Hand::from(cards.as_slice()),
            };
            rules.apply(&mut state, Action::LayMeld(m), &mut rng, &mut events);
        }
        rules.apply(&mut state, Action::Pass, &mut rng, &mut events);
        // Opponent has a 3-card run 5S 6S 7S and a 3-set 10C 10D 10H plus 4S.
        // They choose to keep everything and pass. Detail (d) says this is
        // legal. Their hand should still contain all 7 cards.
        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        assert!(actions.contains(&Action::Pass));
        rules.apply(&mut state, Action::Pass, &mut rng, &mut events);
        assert_eq!(state.phase, Phase::Ended);
        assert_eq!(state.hands[1].len(), 7);
    }

    #[test]
    fn non_closer_can_add_to_tabled_meld() {
        let (rules, mut state, mut events) = setup_closed_hand();
        let mut rng = chance(0);
        // Closer lays the 1s and 2s, then passes.
        for cards in [
            [c(Suit::Clubs, 1), c(Suit::Diamonds, 1), c(Suit::Hearts, 1)],
            [c(Suit::Clubs, 2), c(Suit::Diamonds, 2), c(Suit::Hearts, 2)],
        ] {
            let m = Meld {
                kind: MeldKind::Set,
                cards: Hand::from(cards.as_slice()),
            };
            rules.apply(&mut state, Action::LayMeld(m), &mut rng, &mut events);
        }
        rules.apply(&mut state, Action::Pass, &mut rng, &mut events);
        // The non-closer can add 4S to the 5S-6S-7S run - but that run isn't
        // on the table (they chose not to lay it). What they CAN do: nothing
        // into these tabled sets (no more 1s or 2s in hand). So all their
        // AddToMeld options should be empty, only LayMeld and Pass remain.
        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, Action::AddToMeld { .. }))
        );
        assert!(actions.iter().any(|a| matches!(a, Action::LayMeld(_))));
        assert!(actions.contains(&Action::Pass));
    }

    // --- scoring, bonuses, elimination ---

    /// Drive a specific closing hand into `Phase::Ended` with the showdown
    /// scripted. Returns the rules, final state and HandResult.
    fn play_showdown_to_end(
        closer_hand_after_close: Hand,
        close_card: Card,
        opp_hand: Hand,
        closer_plays: &[Action],
        opp_plays: &[Action],
    ) -> (StandardConga, State, Option<HandResult>) {
        let rules = StandardConga::new();
        let state = State {
            seats_count: 2,
            // Before Close(close_card): closer has closer_hand_after_close + close_card.
            hands: vec![
                {
                    let mut h = closer_hand_after_close;
                    h.insert(close_card);
                    h
                },
                opp_hand,
            ],
            stock: Vec::new(),
            discard_pile: vec![c(Suit::Clubs, 9)],
            scores: vec![0, 0],
            dealer: Seat::new(1),
            current_seat: Seat::new(0),
            phase: Phase::Discard,
            melds_on_table: Vec::new(),
            first_turn_done: vec![true, true],
            must_not_discard: None,
            closer: None,
            closer_hand_at_close: None,
            closer_has_opened: false,
            showdown_consecutive_passes: 0,
            face_down_close: None,
        };
        let mut rules_state = state;
        let mut events = Vec::new();
        let mut rng = chance(0);
        rules.apply(
            &mut rules_state,
            Action::Close(close_card),
            &mut rng,
            &mut events,
        );
        for act in closer_plays {
            rules.apply(&mut rules_state, act.clone(), &mut rng, &mut events);
        }
        // Closer passes.
        rules.apply(&mut rules_state, Action::Pass, &mut rng, &mut events);
        for act in opp_plays {
            rules.apply(&mut rules_state, act.clone(), &mut rng, &mut events);
        }
        // Opponent passes.
        rules.apply(&mut rules_state, Action::Pass, &mut rng, &mut events);
        let result = rules.hand_result(&rules_state);
        (rules, rules_state, result)
    }

    #[test]
    fn score_regular_close_has_no_bonus_closer_pays_unmatched() {
        // Closer: 1C 1D 1H 2C 2D 2H 3S + closes with 9C. Residual unmatched
        // is {3S}, value 3. Opponent: 5S 6S 7S 10C 10D 10H + 4S (keeps all).
        let closer_residual: Hand = [
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Clubs, 2),
            c(Suit::Diamonds, 2),
            c(Suit::Hearts, 2),
            c(Suit::Spades, 3),
        ]
        .as_slice()
        .into();
        let opp_hand: Hand = [
            c(Suit::Spades, 5),
            c(Suit::Spades, 6),
            c(Suit::Spades, 7),
            c(Suit::Clubs, 10),
            c(Suit::Diamonds, 10),
            c(Suit::Hearts, 10),
            c(Suit::Spades, 4),
        ]
        .as_slice()
        .into();

        let set_ones = Action::LayMeld(Meld {
            kind: MeldKind::Set,
            cards: Hand::from(
                [c(Suit::Clubs, 1), c(Suit::Diamonds, 1), c(Suit::Hearts, 1)].as_slice(),
            ),
        });
        let set_twos = Action::LayMeld(Meld {
            kind: MeldKind::Set,
            cards: Hand::from(
                [c(Suit::Clubs, 2), c(Suit::Diamonds, 2), c(Suit::Hearts, 2)].as_slice(),
            ),
        });
        let (_, _, result) = play_showdown_to_end(
            closer_residual,
            c(Suit::Clubs, 9),
            opp_hand,
            &[set_ones, set_twos],
            &[], // opponent keeps everything
        );
        let r = result.unwrap();
        assert_eq!(r.closer, Some(Seat::new(0)));
        assert_eq!(r.bonus, None);
        assert_eq!(r.points[0], 3); // 3S left unmatched
        // Opponent kept all 7 cards: 5+6+7+10+10+10+4 = 52.
        assert_eq!(r.points[1], 5 + 6 + 7 + 10 + 10 + 10 + 4);
    }

    #[test]
    fn score_zero_unmatched_close_gives_closer_minus_ten() {
        // Closer: 1C 1D 1H + run 5S-6S-7S-8S = 7 cards all melded. Closes
        // with 9C face-down.
        let closer_residual: Hand = [
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Spades, 5),
            c(Suit::Spades, 6),
            c(Suit::Spades, 7),
            c(Suit::Spades, 8),
        ]
        .as_slice()
        .into();
        let opp_hand: Hand = [
            c(Suit::Clubs, 2),
            c(Suit::Diamonds, 2),
            c(Suit::Hearts, 2),
            c(Suit::Clubs, 4),
            c(Suit::Diamonds, 4),
            c(Suit::Hearts, 4),
            c(Suit::Spades, 3),
        ]
        .as_slice()
        .into();

        let set_ones = Action::LayMeld(Meld {
            kind: MeldKind::Set,
            cards: Hand::from(
                [c(Suit::Clubs, 1), c(Suit::Diamonds, 1), c(Suit::Hearts, 1)].as_slice(),
            ),
        });
        let run_spades = Action::LayMeld(Meld {
            kind: MeldKind::Run,
            cards: Hand::from(
                [
                    c(Suit::Spades, 5),
                    c(Suit::Spades, 6),
                    c(Suit::Spades, 7),
                    c(Suit::Spades, 8),
                ]
                .as_slice(),
            ),
        });
        let (_, _, result) = play_showdown_to_end(
            closer_residual,
            c(Suit::Clubs, 9),
            opp_hand,
            &[set_ones, run_spades],
            &[],
        );
        let r = result.unwrap();
        assert_eq!(r.closer, Some(Seat::new(0)));
        assert_eq!(r.bonus, Some(Bonus::MinusTen));
        // Closer: 0 points for residual (nothing in hand) + (-10) bonus.
        assert_eq!(r.points[0], -10);
        assert_eq!(r.points[1], 2 + 2 + 2 + 4 + 4 + 4 + 3);
    }

    #[test]
    fn score_conga_close_gives_closer_minus_fifty() {
        // Closer: 1C 2C 3C 4C 5C 6C 7C = 7-card run of Clubs. Closes with 9D.
        let closer_residual: Hand = (1u8..=7)
            .map(|r| c(Suit::Clubs, r))
            .collect::<Vec<_>>()
            .as_slice()
            .into();
        let opp_hand: Hand = [
            c(Suit::Spades, 5),
            c(Suit::Spades, 6),
            c(Suit::Spades, 7),
            c(Suit::Clubs, 10),
            c(Suit::Diamonds, 10),
            c(Suit::Hearts, 10),
            c(Suit::Spades, 4),
        ]
        .as_slice()
        .into();

        let run = Action::LayMeld(Meld {
            kind: MeldKind::Run,
            cards: closer_residual,
        });
        let (_, _, result) =
            play_showdown_to_end(closer_residual, c(Suit::Diamonds, 9), opp_hand, &[run], &[]);
        let r = result.unwrap();
        assert_eq!(r.closer, Some(Seat::new(0)));
        assert_eq!(r.bonus, Some(Bonus::MinusFifty));
        assert_eq!(r.points[0], -50);
        assert_eq!(r.points[1], 5 + 6 + 7 + 10 + 10 + 10 + 4);
    }

    #[test]
    fn hand_result_is_none_while_not_ended() {
        let rules = StandardConga::new();
        let mut rng = chance(1);
        let state = rules.new_hand(&[0, 0], Seat::new(0), &mut rng);
        assert!(rules.hand_result(&state).is_none());
    }

    #[test]
    fn is_conga_detects_7_card_runs_in_every_suit_and_every_start() {
        for suit in Suit::ALL {
            for start in 1u8..=4 {
                let hand: Hand = (start..start + 7)
                    .map(|r| c(suit, r))
                    .collect::<Vec<_>>()
                    .as_slice()
                    .into();
                assert!(
                    StandardConga::is_conga(hand),
                    "expected Conga for suit {suit:?} starting at rank {start}"
                );
            }
        }
    }

    #[test]
    fn is_conga_rejects_non_conga_hands() {
        // 7 cards same suit but not consecutive.
        let hand: Hand = [
            c(Suit::Spades, 1),
            c(Suit::Spades, 2),
            c(Suit::Spades, 3),
            c(Suit::Spades, 4),
            c(Suit::Spades, 5),
            c(Suit::Spades, 6),
            c(Suit::Spades, 8),
        ]
        .as_slice()
        .into();
        assert!(!StandardConga::is_conga(hand));

        // 7 cards consecutive but mixed suits.
        let hand: Hand = [
            c(Suit::Spades, 1),
            c(Suit::Clubs, 2),
            c(Suit::Spades, 3),
            c(Suit::Spades, 4),
            c(Suit::Spades, 5),
            c(Suit::Spades, 6),
            c(Suit::Spades, 7),
        ]
        .as_slice()
        .into();
        assert!(!StandardConga::is_conga(hand));
    }

    // --- match_status ---

    #[test]
    fn match_status_in_progress_when_multiple_under_limit() {
        let rules = StandardConga::new();
        assert_eq!(rules.match_status(&[10, 20, 30]), MatchStatus::InProgress);
        assert_eq!(
            rules.match_status(&[0, 0, 100, 150]),
            MatchStatus::InProgress
        );
    }

    #[test]
    fn match_status_exactly_100_still_in() {
        let rules = StandardConga::new();
        assert_eq!(
            rules.match_status(&[100, 101]),
            MatchStatus::Winner(Seat::new(0))
        );
    }

    #[test]
    fn match_status_last_remaining_wins() {
        let rules = StandardConga::new();
        assert_eq!(
            rules.match_status(&[200, 50, 300]),
            MatchStatus::Winner(Seat::new(1))
        );
    }

    #[test]
    fn match_status_all_eliminated_lowest_score_wins() {
        let rules = StandardConga::new();
        assert_eq!(
            rules.match_status(&[150, 110, 300]),
            MatchStatus::Winner(Seat::new(1))
        );
    }

    #[test]
    fn match_status_tie_breaks_by_seat_order_on_equal_lowest() {
        let rules = StandardConga::new();
        // Seat 1 and seat 3 tie at 110; seat 1 wins by seat index.
        assert_eq!(
            rules.match_status(&[200, 110, 150, 110]),
            MatchStatus::Winner(Seat::new(1))
        );
    }

    #[test]
    fn showdown_accepts_add_to_meld_extending_a_run() {
        let rules = StandardConga::new();
        // Build a showdown state where a tabled run 5S-6S-7S exists and the
        // acting seat holds 4S and 8S.
        let acting_hand: Hand = [
            c(Suit::Spades, 4),
            c(Suit::Spades, 8),
            c(Suit::Clubs, 10),
            c(Suit::Diamonds, 10),
            c(Suit::Hearts, 10),
            c(Suit::Clubs, 7),
            c(Suit::Diamonds, 7),
        ]
        .as_slice()
        .into();
        let tabled_run = Meld {
            kind: MeldKind::Run,
            cards: Hand::from(
                [c(Suit::Spades, 5), c(Suit::Spades, 6), c(Suit::Spades, 7)].as_slice(),
            ),
        };
        let state = State {
            seats_count: 2,
            hands: vec![acting_hand, Hand::EMPTY],
            stock: Vec::new(),
            discard_pile: vec![c(Suit::Clubs, 9)],
            scores: vec![0, 0],
            dealer: Seat::new(1),
            current_seat: Seat::new(0),
            phase: Phase::Showdown,
            melds_on_table: vec![tabled_run],
            first_turn_done: vec![true, true],
            must_not_discard: None,
            closer: Some(Seat::new(1)),
            closer_hand_at_close: Some(Hand::EMPTY),
            closer_has_opened: true,
            showdown_consecutive_passes: 0,
            face_down_close: None,
        };
        let mut actions = ActionList::new();
        rules.legal_actions(&state, &mut actions);
        let adds: Vec<_> = actions
            .iter()
            .filter_map(|a| match a {
                Action::AddToMeld { card, meld_index } => Some((*card, *meld_index)),
                _ => None,
            })
            .collect();
        assert!(adds.contains(&(c(Suit::Spades, 4), 0)));
        assert!(adds.contains(&(c(Suit::Spades, 8), 0)));
        // 10C, 10D, 10H, 7C, 7D don't extend the run.
        assert!(!adds.iter().any(|(card, _)| card.rank().as_u8() == 10));
    }
}
