# Progress

Claude updates this file at every stop. A new session reads it first.

## Now

- Milestone 4: belief tracker + search agent. Plan at `docs/plans/m4.md`. Branch `m4-belief-search` from `main` (tag `m3-accepted`).
- Slice 1 committed (`c9b075e`): `StandardConga::sample_state` + conformance test 5.
- Slice 2 committed (`225159b`): `BeliefTracker` module (notify / known_held / prob_holds / sample_state).
- Slice 3 committed (`d8ced62`): `SearchPlayer` (PIMC, shared determinization) + `BaselineAgent::Search` + `conga-cli eval/play` `--samples` and `play --ai` flags.
- Slice 3 diagnostics committed (`6f15f7b`): rollout terminal counters + `RolloutPolicy` / `opp_weight` knobs + 2 ignored experiment tests. 281 tests pass, 7 with `--ignored`.
- **Pivot (2026-11-20, D37):** PIMC-with-greedy-rollouts loses to greedy (4% default, 0% random rollouts, 12.5% opp_weight 0.3). Structural strategy fusion. `SearchPlayer` ships as scaffolding (beats random 100-0); standalone search deferred to m7 with the learned value model.
- Slice 4 committed (`ab22b37`): rule-based v2 using BeliefTracker. Three 200-game smokes vs greedy (seeds 1, 0xC0FFEE, 0xBEEF): 35.5% / 37.0% / 34.0%. Statistically indistinguishable from D28's 36-38%. Kept as a refactor per D38.
- Slice 5 complete: all `#[ignore]`d regressions re-run green (`sample_state` 1.9s, `belief` 0.8s, `search` smoke 26.0s at 239 rollouts/sec, m3 `exit_regressions` 44.8s). m5 kickoff prompt written at `docs/prompts/05-milestone-5.md`. **Milestone 4 awaiting Simone's acceptance.**
- Last accepted tag: `m3-accepted`

## Done

- Session 0: repository setup, tagged `setup-accepted` on commit 21f9b43.
- Milestone 0: the six showdown details a to f confirmed (decisions D8 to D13).
- Milestone 1: core engine. Merged into `main` on 2026-11-17 and tagged `m1-accepted`. Branch `m1-core-engine` kept for history. 11 slice commits plus one post-playthrough fix (C3): plan + cards/hands/seats + meld solver + action/event/observation types + RuleSet/Chance/conformance + StandardConga playing phase + showdown + scoring with −10 and −50 bonuses + event log and exact replay + debug CLI, then the starter-dealt-8 clarification caught during the playthrough. 84 tests pass (fmt, clippy -D warnings, 300-game replay invariant). Rule clarifications C1, C2, C3 encoded in the engine and logged in `docs/design.md`; decisions D14, D15, D16, D17 logged in `docs/decisions.md`.
- Milestone 2: game server, protocol, in-memory transport, TUI. Merged into `main` on 2026-11-18 and tagged `m2-accepted`. Branch `m2-server-tui` kept for history. 12 slice commits: serde on protocol-visible types + `conga-protocol` crate + `conga-server` Transport + MatchState / JSON-lines log / hand orchestrator + `conga-ai` crate with `Player` trait and `RandomPlayer` + `conga-tui` terminal client + `conga-cli` play/replay subcommands, then a TUI redesign (slices 8–12) replacing the numbered action list with card-tile rendering, a live cursor, meld-cycle mode and manual card reorder. 220 tests pass. Decisions D18–D25 logged in `docs/decisions.md`.
- Milestone 3: baseline agents + evaluation harness. Merged into `main` on 2026-11-19 and tagged `m3-accepted`. Branch `m3-baselines-eval` kept for history. 7 slice commits plus one lockfile cleanup: `Player` sync + `GreedyPlayer` (D27) + `RuleBasedPlayer` (D28) + headless runner with paired deals (D29) + `EvalReport` with Wilson CI and `conga-cli eval` + stalemate terminal `HandTerminal { Closed, Stalemate }` (D31, C4) + 2000-game `#[ignore]`d regression. The planned `rule_based_beats_greedy` assertion was dropped: three seed smokes showed D28 loses to greedy at 36–38%, so the shipped regression asserts `rule_based_beats_random` (100%) instead. 261 tests pass (263 with `--ignored`). Decisions D26–D31 logged in `docs/decisions.md`; rule clarification C4 in `docs/design.md`.

## Open questions for Simone

None.

## Deviations from the design doc

- D14: `RuleSet::sample_state` signature uses `&mut dyn Rng` (rand_core 0.10 deprecated `RngCore`). Design doc updated in the same commit.
- D31: A new slice 6 (stalemate terminal) was inserted into the m3 plan ahead of the originally-numbered slice 6; exit-test regression is now slice 7. Reason: slice 5 playtesting surfaced an infinite-loop in two deterministic agents that required an engine-level fix before the regression could run.
- Slice 7 regression replaced `rule_based_beats_greedy` with `rule_based_beats_random`. Three seed smokes (200 games each) showed rule-based consistently losing to greedy at 36–38%; the D28 safety heuristic underperforms plain greedy against a deterministic opponent. Shipping with the truthful regression; D28 rework is deferred.

## Next

- Milestone 4: belief tracker + search agent. Rule-based v2 (a working D28 successor) is rolled into m4 alongside the belief tracker, since the natural signal for a stronger rule-based agent (what the opponent likely holds) is exactly what the belief tracker produces.
