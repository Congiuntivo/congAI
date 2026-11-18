//! `AppState`: everything the renderer needs. The driver updates it on
//! each server message; `render::draw` reads it.

use conga_core::{Action, Card, Event, Hand, HandResult, MatchStatus, Observation};

use crate::suits::SuitTheme;

const MAX_EVENTS_SHOWN: usize = 6;

/// Where the TUI cursor sits. The tile renderer highlights it with a
/// double border; the driver resolves key intents against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorPos {
    Stock,
    Discard,
    Hand(usize),
}

/// Interaction mode. `Normal` is cursor-driven (slice 10). The cycle
/// modes hold a slice of already-filtered legal actions plus the index
/// of the one currently under the ← → cycle (slice 11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    LayMeldCycle { variants: Vec<Action>, index: usize },
    AddToMeldCycle { variants: Vec<Action>, index: usize },
}

/// Rebuild a display order for the hand: keep cards from `prior` that
/// are still in `new_hand` (preserving their relative order), then
/// append the cards in `new_hand` that weren't in `prior` (in the
/// hand's canonical iteration order). Pure so it's unit-testable.
pub fn rebuild_hand_order(prior: &[Card], new_hand: Hand) -> Vec<Card> {
    let mut next: Vec<Card> = prior
        .iter()
        .copied()
        .filter(|c| new_hand.contains(*c))
        .collect();
    for c in new_hand.iter() {
        if !next.contains(&c) {
            next.push(c);
        }
    }
    next
}

/// Choose a reasonable starting cursor for a new `AwaitAction`: Stock if
/// drawing from stock is legal, Discard if only drawing from the top
/// discard is legal, else the first hand card. `hand_size` is the
/// clamping bound for the Hand variant; an empty hand with no draw
/// options falls back to Stock (unreachable in a well-formed game, but
/// keeps the function total).
pub fn initial_cursor(legal: &[Action], hand_size: usize) -> CursorPos {
    if legal.iter().any(|a| matches!(a, Action::DrawStock)) {
        CursorPos::Stock
    } else if legal.iter().any(|a| matches!(a, Action::DrawDiscard)) {
        CursorPos::Discard
    } else if hand_size > 0 {
        CursorPos::Hand(0)
    } else {
        CursorPos::Stock
    }
}

pub struct AppState {
    pub own_seat: u8,
    pub seats_count: u8,
    pub theme: SuitTheme,
    pub hand_index: u32,
    pub latest_turn: u32,
    /// `None` until the server sends the first `Observe`.
    pub observation: Option<Observation>,
    /// `Some` only while an `AwaitAction` is live and the observation
    /// matches the current seat. Empty otherwise.
    pub legal: Vec<Action>,
    /// Which tile the cursor points at. Reset to `initial_cursor` when
    /// `AwaitAction` arrives.
    pub cursor: CursorPos,
    /// Display order of the own hand. Rebuilt from each `Observe` so
    /// cards the player has already reordered keep their positions;
    /// newly-drawn cards are appended. `Shift+←/→` swap neighbors.
    pub hand_order: Vec<Card>,
    /// Interaction mode: `Normal` for cursor-driven play, cycle modes
    /// for the meld builder.
    pub mode: Mode,
    /// Most recent HandResult, shown briefly between hands.
    pub last_hand_result: Option<HandResult>,
    pub match_status: Option<MatchStatus>,
    /// Status-line text: last error from the server, or a help string.
    pub status_line: String,
    /// Ring of the last `MAX_EVENTS_SHOWN` events.
    pub recent_events: Vec<Event>,
}

impl AppState {
    pub fn new(own_seat: u8, seats_count: u8, theme: SuitTheme) -> Self {
        AppState {
            own_seat,
            seats_count,
            theme,
            hand_index: 0,
            latest_turn: 0,
            observation: None,
            legal: Vec::new(),
            cursor: CursorPos::Stock,
            hand_order: Vec::new(),
            mode: Mode::Normal,
            last_hand_result: None,
            match_status: None,
            status_line: "waiting for the first observation...".into(),
            recent_events: Vec::new(),
        }
    }

    fn hand_size(&self) -> usize {
        self.hand_order.len()
    }

    /// The hand card under the cursor, or `None` if the cursor is on
    /// Stock / Discard or past the end of the hand.
    pub fn hand_card_at_cursor(&self) -> Option<Card> {
        match self.cursor {
            CursorPos::Hand(i) => self.hand_order.get(i).copied(),
            _ => None,
        }
    }

    /// Rebuild `hand_order` from the server's new hand. Cards that
    /// were already in the display order keep their places; cards no
    /// longer in the hand are dropped; newly-dealt cards are appended.
    pub fn update_hand_order(&mut self, new_hand: Hand) {
        self.hand_order = rebuild_hand_order(&self.hand_order, new_hand);
    }

    /// Swap the selected card with its left neighbor; the cursor
    /// follows the card. No-op on non-hand cursors or at the left
    /// edge.
    pub fn swap_cursor_left(&mut self) {
        if let CursorPos::Hand(i) = self.cursor
            && i > 0
            && i < self.hand_order.len()
        {
            self.hand_order.swap(i - 1, i);
            self.cursor = CursorPos::Hand(i - 1);
        }
    }

    /// Swap the selected card with its right neighbor; the cursor
    /// follows the card. No-op on non-hand cursors or at the right
    /// edge.
    pub fn swap_cursor_right(&mut self) {
        if let CursorPos::Hand(i) = self.cursor
            && i + 1 < self.hand_order.len()
        {
            self.hand_order.swap(i, i + 1);
            self.cursor = CursorPos::Hand(i + 1);
        }
    }

    /// Move the cursor one tile left. Stops at Stock.
    pub fn move_cursor_left(&mut self) {
        self.cursor = match self.cursor {
            CursorPos::Stock => CursorPos::Stock,
            CursorPos::Discard => CursorPos::Stock,
            CursorPos::Hand(0) => CursorPos::Discard,
            CursorPos::Hand(i) => CursorPos::Hand(i - 1),
        };
    }

    /// Move the cursor one tile right. Stops at the last hand card.
    pub fn move_cursor_right(&mut self) {
        let hand_size = self.hand_size();
        self.cursor = match self.cursor {
            CursorPos::Stock => CursorPos::Discard,
            CursorPos::Discard => {
                if hand_size > 0 {
                    CursorPos::Hand(0)
                } else {
                    CursorPos::Discard
                }
            }
            CursorPos::Hand(i) => {
                if i + 1 < hand_size {
                    CursorPos::Hand(i + 1)
                } else {
                    CursorPos::Hand(i)
                }
            }
        };
    }

    /// Reset cursor for a fresh `AwaitAction`, picking the first slot
    /// that has a legal action.
    pub fn reset_cursor(&mut self) {
        self.cursor = initial_cursor(&self.legal, self.hand_size());
    }

    /// Enter `LayMeldCycle` with the current legal `LayMeld` actions, if
    /// any. Returns `true` on entry, `false` if there is nothing to
    /// cycle (driver writes a status-line hint in that case).
    pub fn enter_lay_meld_mode(&mut self) -> bool {
        let variants: Vec<Action> = self
            .legal
            .iter()
            .filter(|a| matches!(a, Action::LayMeld(_)))
            .cloned()
            .collect();
        if variants.is_empty() {
            return false;
        }
        self.mode = Mode::LayMeldCycle { variants, index: 0 };
        true
    }

    /// Enter `AddToMeldCycle` with the current legal `AddToMeld`
    /// actions. Returns `false` if the player has nothing to add.
    pub fn enter_add_meld_mode(&mut self) -> bool {
        let variants: Vec<Action> = self
            .legal
            .iter()
            .filter(|a| matches!(a, Action::AddToMeld { .. }))
            .cloned()
            .collect();
        if variants.is_empty() {
            return false;
        }
        self.mode = Mode::AddToMeldCycle { variants, index: 0 };
        true
    }

    /// Advance the cycle index by one with wrap-around.
    pub fn cycle_next(&mut self) {
        match &mut self.mode {
            Mode::LayMeldCycle { variants, index } | Mode::AddToMeldCycle { variants, index } => {
                if !variants.is_empty() {
                    *index = (*index + 1) % variants.len();
                }
            }
            Mode::Normal => {}
        }
    }

    /// Step the cycle index back by one with wrap-around.
    pub fn cycle_prev(&mut self) {
        match &mut self.mode {
            Mode::LayMeldCycle { variants, index } | Mode::AddToMeldCycle { variants, index } => {
                if !variants.is_empty() {
                    *index = (*index + variants.len() - 1) % variants.len();
                }
            }
            Mode::Normal => {}
        }
    }

    /// Clone the `Action` at the current cycle index. `None` outside a
    /// cycle mode.
    pub fn current_cycle_action(&self) -> Option<Action> {
        match &self.mode {
            Mode::LayMeldCycle { variants, index } | Mode::AddToMeldCycle { variants, index } => {
                variants.get(*index).cloned()
            }
            Mode::Normal => None,
        }
    }

    /// Cards the current cycle action touches. For LayMeld, every card
    /// of the meld; for AddToMeld, the single card being added. Empty
    /// outside cycle modes.
    pub fn current_cycle_cards(&self) -> Vec<Card> {
        match self.current_cycle_action() {
            Some(Action::LayMeld(m)) => m.cards.iter().collect(),
            Some(Action::AddToMeld { card, .. }) => vec![card],
            _ => Vec::new(),
        }
    }

    /// Leave any cycle mode and return to `Normal`.
    pub fn exit_mode(&mut self) {
        self.mode = Mode::Normal;
    }

    pub fn is_in_cycle_mode(&self) -> bool {
        !matches!(self.mode, Mode::Normal)
    }

    pub fn push_event(&mut self, event: Event) {
        self.recent_events.push(event);
        if self.recent_events.len() > MAX_EVENTS_SHOWN {
            let overflow = self.recent_events.len() - MAX_EVENTS_SHOWN;
            self.recent_events.drain(0..overflow);
        }
    }

    pub fn clear_legal(&mut self) {
        self.legal.clear();
        self.mode = Mode::Normal;
    }

    pub fn is_my_turn(&self) -> bool {
        self.observation
            .as_ref()
            .is_some_and(|o| o.current_seat.index() == self.own_seat)
            && !self.legal.is_empty()
    }
}
