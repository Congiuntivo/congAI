# Progress

Claude updates this file at every stop. A new session reads it first.

## Now

- Milestone: 3 (baseline agents + evaluation harness)
- Slice: 6 (stalemate terminal, D31) — done, awaiting review
- Branch: `m3-baselines-eval`
- Last accepted tag: `m2-accepted`
- Plan: `docs/plans/m3.md` (approved 2026-11-18; slice 6 inserted 2026-11-18)
- Slice 6 adds `HandTerminal { Closed, Stalemate }` to `conga-core`, caps `StandardConga` at 3 reshuffles per hand (4th attempt flips `stalemate_pending`), replaces the deprecated `unreachable!` on empty-stock + too-small-pile with the same stalemate flag, and surfaces a `Stalemate` hand result (all seats score 0). Fixes the hang found in slice 5 playtesting where two deterministic greedy-style agents could cycle forever. Logged as D31; rule clarification C4 in `docs/design.md`. Previously-hanging `greedy vs greedy, 50 games, seed 1` now runs in 4.7s (25-25, 13.36 hands/match); `rule-based vs greedy, 50 games` runs in 5.1s. 261 tests pass (+8 new: 4 engine, 2 AI termination, 2 serde).

## Done

- Session 0: repository setup, tagged `setup-accepted` on commit 21f9b43.
- Milestone 0: the six showdown details a to f confirmed (decisions D8 to D13).
- Milestone 1: core engine. Merged into `main` on 2026-11-17 and tagged `m1-accepted`. Branch `m1-core-engine` kept for history. 11 slice commits plus one post-playthrough fix (C3): plan + cards/hands/seats + meld solver + action/event/observation types + RuleSet/Chance/conformance + StandardConga playing phase + showdown + scoring with −10 and −50 bonuses + event log and exact replay + debug CLI, then the starter-dealt-8 clarification caught during the playthrough. 84 tests pass (fmt, clippy -D warnings, 300-game replay invariant). Rule clarifications C1, C2, C3 encoded in the engine and logged in `docs/design.md`; decisions D14, D15, D16, D17 logged in `docs/decisions.md`.
- Milestone 2: game server, protocol, in-memory transport, TUI. Merged into `main` on 2026-11-18 and tagged `m2-accepted`. Branch `m2-server-tui` kept for history. 12 slice commits: serde on protocol-visible types + `conga-protocol` crate + `conga-server` Transport + MatchState / JSON-lines log / hand orchestrator + `conga-ai` crate with `Player` trait and `RandomPlayer` + `conga-tui` terminal client + `conga-cli` play/replay subcommands, then a TUI redesign (slices 8–12) replacing the numbered action list with card-tile rendering, a live cursor, meld-cycle mode and manual card reorder. 220 tests pass. Decisions D18–D25 logged in `docs/decisions.md`.

## Open questions for Simone

None.

## Deviations from the design doc

- D14: `RuleSet::sample_state` signature uses `&mut dyn Rng` (rand_core 0.10 deprecated `RngCore`). Design doc updated in the same commit.
- D31: A new slice 6 (stalemate terminal) was inserted into the m3 plan ahead of the originally-numbered slice 6; exit-test regression is now slice 7. Reason: slice 5 playtesting surfaced an infinite-loop in two deterministic agents that required an engine-level fix before the regression could run.

## Next

- Milestone 3, slice 7: exit-test regression (2000-game greedy>random and rule-based>greedy, Wilson lower bound > 0.5). Slice 6's stalemate fix unblocks the rule-based side.
