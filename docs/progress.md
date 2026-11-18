# Progress

Claude updates this file at every stop. A new session reads it first.

## Now

- Milestone: 2 (game server, protocol, in-memory transport, TUI)
- Slice: 10 (TUI redesign — cursor + simple actions) — done, awaiting review
- Branch: `m2-server-tui`
- Last accepted tag: `m1-accepted`
- Plan: `docs/plans/m2.md` (approved 2026-11-17; TUI redesign slices 8–12 appended 2026-11-18)
- Slices 1–9 committed. Slice 10 wires the live cursor: `AppState.cursor`, `move_cursor_left/right`, `initial_cursor` picks Stock / Discard / Hand(0) based on legal actions. New `actions.rs` resolver maps `(KeyIntent, CursorPos, legal)` → `Resolved::{Accept, Reject, MoveLeft, MoveRight, Quit, NoOp}`. Input: `←/→` move, `Enter` acts on the hovered tile (Stock → DrawStock, Discard → DrawDiscard, Hand(i) → Discard(hand[i])), `d/c/p` discard / close / pass, `q`/`Esc` quit. The numbered 1..0 menu is gone; a bindings footer replaces it, greying out illegal keys. 197 tests pass.

## Done

- Session 0: repository setup, tagged `setup-accepted` on commit 21f9b43.
- Milestone 0: the six showdown details a to f confirmed (decisions D8 to D13).
- Milestone 1: core engine. Merged into `main` on 2026-11-17 and tagged `m1-accepted`. Branch `m1-core-engine` kept for history. 11 slice commits plus one post-playthrough fix (C3): plan + cards/hands/seats + meld solver + action/event/observation types + RuleSet/Chance/conformance + StandardConga playing phase + showdown + scoring with −10 and −50 bonuses + event log and exact replay + debug CLI, then the starter-dealt-8 clarification caught during the playthrough. 84 tests pass (fmt, clippy -D warnings, 300-game replay invariant). Rule clarifications C1, C2, C3 encoded in the engine and logged in `docs/design.md`; decisions D14, D15, D16, D17 logged in `docs/decisions.md`.

## Open questions for Simone

None.

## Deviations from the design doc

- D14: `RuleSet::sample_state` signature uses `&mut dyn Rng` (rand_core 0.10 deprecated `RngCore`). Design doc updated in the same commit.

## Next

- Simone's milestone-2 exit-test playthrough. Once accepted, merge `m2-server-tui` into `main`, tag `m2-accepted`, then start milestone 3 (baseline agents + evaluation harness).
