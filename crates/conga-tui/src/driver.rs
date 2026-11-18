//! TUI event loop and terminal lifecycle.
//!
//! `run_tui` owns the terminal (raw mode + alternate screen) until the
//! match ends. Terminal restore runs on drop so a panic does not leave
//! the user's shell in a weird state.

use std::fmt;
use std::io::{self, Stdout, stdout};

use conga_core::Action;
use conga_protocol::{ClientToServer, ServerToClient};
use conga_server::{Transport, TransportError};
use crossterm::event::{Event as CtEvent, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{event, execute};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::actions::{CycleResolved, Resolved, resolve, resolve_cycle};
use crate::app::AppState;
use crate::input::key_to_intent;
use crate::render::{cycle_status_line, draw};
use crate::suits::SuitTheme;

#[derive(Debug)]
pub enum TuiError {
    Transport(TransportError),
    Io(io::Error),
    UserQuit,
}

impl fmt::Display for TuiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TuiError::Transport(e) => write!(f, "transport: {e}"),
            TuiError::Io(e) => write!(f, "io: {e}"),
            TuiError::UserQuit => f.write_str("user quit (q)"),
        }
    }
}

impl std::error::Error for TuiError {}

impl From<TransportError> for TuiError {
    fn from(value: TransportError) -> Self {
        TuiError::Transport(value)
    }
}

impl From<io::Error> for TuiError {
    fn from(value: io::Error) -> Self {
        TuiError::Io(value)
    }
}

/// RAII guard: enter raw mode + alternate screen on `new`, restore on
/// drop. Keeps the terminal usable even if the loop panics.
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(out);
        let terminal = Terminal::new(backend)?;
        Ok(TerminalGuard { terminal })
    }

    fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best-effort restore; errors are ignored because we are already
        // tearing down.
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

/// Read one key event, discarding non-key events. Runs the blocking
/// crossterm call on a dedicated blocking task.
async fn next_key() -> io::Result<crossterm::event::KeyEvent> {
    tokio::task::spawn_blocking(|| {
        loop {
            match event::read()? {
                CtEvent::Key(k) if k.kind != KeyEventKind::Release => return Ok(k),
                _ => continue,
            }
        }
    })
    .await
    .map_err(io::Error::other)?
}

/// Drive the TUI client until the server sends `MatchEnded` or the
/// user presses `q`.
pub async fn run_tui<T>(
    mut transport: T,
    own_seat: u8,
    seats_count: u8,
    theme: SuitTheme,
) -> Result<(), TuiError>
where
    T: Transport<Outgoing = ClientToServer, Incoming = ServerToClient> + Send,
{
    let mut guard = TerminalGuard::new()?;
    let mut state = AppState::new(own_seat, seats_count, theme);

    guard.terminal().draw(|f| draw(f, &state))?;

    loop {
        let msg = transport.recv().await?;
        match msg {
            ServerToClient::Observe { turn, observation } => {
                state.latest_turn = turn;
                state.observation = Some(observation);
                state.clear_legal();
            }
            ServerToClient::AwaitAction { legal, .. } => {
                state.legal = legal;
                state.reset_cursor();
                state.exit_mode();
                state.status_line =
                    "your turn — ← → move, Enter act, c close, m meld, a add, p pass, q quit"
                        .into();
                let action = read_action(&mut guard, &mut state).await?;
                transport
                    .send(ClientToServer::Act {
                        turn: state.latest_turn,
                        action,
                    })
                    .await?;
                state.clear_legal();
                state.status_line = "waiting...".into();
            }
            ServerToClient::Event { event, .. } => {
                state.push_event(event);
            }
            ServerToClient::HandEnded { result } => {
                state.hand_index = state.hand_index.saturating_add(1);
                state.status_line = format!("hand ended: {:?}", result.points);
                state.last_hand_result = Some(result);
                state.recent_events.clear();
            }
            ServerToClient::MatchEnded {
                status,
                final_scores,
            } => {
                state.match_status = Some(status);
                state.status_line =
                    format!("match ended: {status:?}  scores: {final_scores:?}  (press any key)");
                guard.terminal().draw(|f| draw(f, &state))?;
                let _ = next_key().await?;
                return Ok(());
            }
            ServerToClient::Error { code, message } => {
                state.status_line = format!("server rejected action [{code:?}]: {message}");
            }
        }
        guard.terminal().draw(|f| draw(f, &state))?;
    }
}

/// Block on key input until the user picks a legal action or quits.
/// The loop dispatches on `state.mode`: Normal mode uses the cursor
/// resolver, cycle modes use the meld-cycle resolver.
async fn read_action(guard: &mut TerminalGuard, state: &mut AppState) -> Result<Action, TuiError> {
    loop {
        guard.terminal().draw(|f| draw(f, state))?;
        let key = next_key().await?;
        let intent = key_to_intent(&key);
        let obs = state
            .observation
            .clone()
            .expect("AwaitAction without a prior Observe");
        if state.is_in_cycle_mode() {
            match resolve_cycle(intent) {
                CycleResolved::Prev => {
                    state.cycle_prev();
                    if let Some(s) = cycle_status_line(state) {
                        state.status_line = s;
                    }
                }
                CycleResolved::Next => {
                    state.cycle_next();
                    if let Some(s) = cycle_status_line(state) {
                        state.status_line = s;
                    }
                }
                CycleResolved::Commit => {
                    if let Some(action) = state.current_cycle_action() {
                        state.exit_mode();
                        return Ok(action);
                    }
                    state.exit_mode();
                }
                CycleResolved::Cancel => {
                    state.exit_mode();
                    state.status_line = "cancelled".into();
                }
                CycleResolved::Quit => return Err(TuiError::UserQuit),
                CycleResolved::Ignore => {}
            }
            continue;
        }
        let resolved = resolve(intent, state.cursor, &obs, &state.legal, state.theme);
        match resolved {
            Resolved::Accept(action) => return Ok(action),
            Resolved::Reject(msg) => state.status_line = msg,
            Resolved::MoveLeft => state.move_cursor_left(),
            Resolved::MoveRight => state.move_cursor_right(),
            Resolved::EnterLayMeld => {
                if state.enter_lay_meld_mode() {
                    if let Some(s) = cycle_status_line(state) {
                        state.status_line = s;
                    }
                } else {
                    state.status_line = "no legal meld to lay right now".into();
                }
            }
            Resolved::EnterAddMeld => {
                if state.enter_add_meld_mode() {
                    if let Some(s) = cycle_status_line(state) {
                        state.status_line = s;
                    }
                } else {
                    state.status_line = "no meld on the table can take a card right now".into();
                }
            }
            Resolved::Cancel => return Err(TuiError::UserQuit),
            Resolved::Quit => return Err(TuiError::UserQuit),
            Resolved::NoOp => {}
        }
    }
}
