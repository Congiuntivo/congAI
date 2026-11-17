# Milestone 2: game server, protocol, in-memory transport, TUI

Start a new Claude Code session in plan mode. Paste the text below the line as the first message.

---

We are starting milestone 2 from `docs/design.md`: the authoritative game server, the protocol, the in-memory transport and the TUI. At the end of this milestone I can play a full match against the random agent in the terminal and replay it from its log.

Milestone 1 is accepted (tag `m1-accepted` on `main`). The engine, the `RuleSet` trait, the meld solver and `StandardConga` all exist. Rule clarifications C1, C2, C3 are encoded; see `docs/design.md` under "Rule clarifications".

## Before the plan

Decisions to settle before slicing. Ask me each and record the answers in `docs/decisions.md`.

1. **Wire format.** Design doc choice 3 says JSON lines. Confirm or propose an alternative.
2. **Random agent — in m2 or m3?** Milestone 3 is "Baseline agents and evaluation harness". The m2 exit test needs the random agent: pull it forward into m2, or stub a minimal random `Player` here and move the full baseline suite to m3.
3. **TUI scope for v1.** The design doc describes local multiplayer with a hand-over screen that hides the previous hand. For the m2 exit test, only "me vs one random AI" is required. Start narrow (single-human vs single-AI) or also build the hand-over screen now.
4. **Match state representation.** The hand-level `State` in `conga-rules` is one hand. The match layer needs scores across hands, dealer rotation (rule 16), and elimination tracking. Propose `MatchState` as a wrapper type in `conga-server`, or elsewhere.
5. **Replay command.** Design doc wants replay. Propose: `conga-cli replay <log-file>` reads a JSON-lines log and prints every state. Confirm or adjust.

## Plan, with no code yet

1. Read `docs/progress.md`, `docs/decisions.md` (D1 to D17), `docs/design.md` (Architecture, Play modes, the Software components rows for `conga-protocol`, `conga-server`, `conga-tui`, Design rules 1-8, the six protocol details), and `docs/plans/m1.md` for the shape of a milestone plan.
2. List what you do not know yet: library APIs for `ratatui` 0.30, `crossterm`, `serde_json`, `tokio`. Milestone 2 does not need `axum` or WebSocket yet; confirm nothing you add here blocks their later integration. Research what you can; bring the rest as questions with options.
3. Propose the slices. For each: goal, files touched, tests, the check that proves it, how I try it by hand.
4. Wait for my approval. Save the plan to `docs/plans/m2.md`, create the branch `m2-server-tui` and start slice 1.

## What the plan must cover

Treat this list as a starting point. Propose a better cut if you see one.

- `conga-protocol`: `Message` enum wrapping the existing `Action`, `Observation` and `Event` types for the wire. Per-seat event sequence numbers (protocol detail 1). Turn number on actions (protocol detail 2). Protocol version and ruleset hash on every message. Serde JSON-line serialization behind a feature flag on `conga-core` or in `conga-protocol` itself. Reserved fields for turn timer and timeout (protocol detail 3).
- `conga-server`: `MatchState` (scores across hands, dealer rotation, elimination, match status). Hand orchestration: `new_hand` -> validated `apply` loop -> `hand_result` -> score update -> `match_status`. Event log with per-seat sequence numbers. The server never trusts a client: every action goes through `legal_actions` before `apply`.
- Transport: one trait with an in-memory channel implementation. The WebSocket implementation waits for milestone 9.
- A random `Player`: samples uniformly from legal actions. Lives in a new `conga-ai` crate (minimally) or in `conga-server` depending on question 2 above.
- `conga-tui`: `ratatui` + `crossterm` client. Renders the acting seat's observation (your own hand grouped by the meld solver's output, like the m1 debug CLI). Reads one action per turn via a selectable numbered menu. No hints, no suggestions, no AI-side reasoning in the TUI itself.
- Match replay: a CLI subcommand that reads a JSON-lines log and reconstructs the match trajectory, using and possibly extending the slice-8 `replay` helper to the match level.

## Exit test

I play a full 2-player match against the random agent in the TUI. The server validates every action. Hand scores and match status agree with the numbers the engine would produce by itself (milestone 1 behaviour). Match ends when a player exceeds 100 (or the rule-16 tie-break kicks in). I replay the saved log via the CLI command and see the same hand-by-hand states I lived through.

Follow the working loop in `CLAUDE.md`: one slice, checks, commit, stop report, then wait for me.
