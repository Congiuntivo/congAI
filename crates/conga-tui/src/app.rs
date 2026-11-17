//! `AppState`: everything the renderer needs. The driver updates it on
//! each server message; `render::draw` reads it.

use conga_core::{Action, Event, HandResult, MatchStatus, Observation};

const MAX_EVENTS_SHOWN: usize = 6;

pub struct AppState {
    pub own_seat: u8,
    pub seats_count: u8,
    pub hand_index: u32,
    pub latest_turn: u32,
    /// `None` until the server sends the first `Observe`.
    pub observation: Option<Observation>,
    /// `Some` only while an `AwaitAction` is live and the observation
    /// matches the current seat. Empty otherwise.
    pub legal: Vec<Action>,
    /// Most recent HandResult, shown briefly between hands.
    pub last_hand_result: Option<HandResult>,
    pub match_status: Option<MatchStatus>,
    /// Status-line text: last error from the server, or a help string.
    pub status_line: String,
    /// Ring of the last `MAX_EVENTS_SHOWN` events.
    pub recent_events: Vec<Event>,
}

impl AppState {
    pub fn new(own_seat: u8, seats_count: u8) -> Self {
        AppState {
            own_seat,
            seats_count,
            hand_index: 0,
            latest_turn: 0,
            observation: None,
            legal: Vec::new(),
            last_hand_result: None,
            match_status: None,
            status_line: "waiting for the first observation...".into(),
            recent_events: Vec::new(),
        }
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
    }

    pub fn is_my_turn(&self) -> bool {
        self.observation
            .as_ref()
            .is_some_and(|o| o.current_seat.index() == self.own_seat)
            && !self.legal.is_empty()
    }
}
