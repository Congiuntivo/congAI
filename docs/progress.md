# Progress

Claude updates this file at every stop. A new session reads it first.

## Now

- Milestone: 2 (game server, protocol, in-memory transport, TUI)
- Slice: 12 (TUI redesign — manual card reorder) — done, awaiting review
- Branch: `m2-server-tui`
- Last accepted tag: `m1-accepted`
- Plan: `docs/plans/m2.md` (approved 2026-11-17; TUI redesign slices 8–12 appended 2026-11-18)
- Slices 1–11 committed. Slice 12 adds manual card reorder. `AppState.hand_order: Vec<Card>` is the display order; `update_hand_order` + `rebuild_hand_order` keep previously-reordered cards in place across `Observe` updates and append new ones at the end. `Shift+←` / `Shift+→` swap the hovered hand card with its neighbor (cursor follows). The strip and the resolver both read `hand_order` rather than the sorted `Hand::iter()`. `resolve` now takes a `hand_order: &[Card]` instead of `&Observation`. 220 tests pass. Milestone 2 is ready for the exit-test playthrough.

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
