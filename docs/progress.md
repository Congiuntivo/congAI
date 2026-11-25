# Progress

Claude updates this file at every stop. A new session reads it first.

## Now

- Milestone 4 accepted on 2026-11-20, tagged `m4-accepted`. Branch `m4-belief-search` kept for history.
- No open slice. Awaiting Simone's go-ahead to plan m5 (match equity + score-aware play; kickoff prompt at `docs/prompts/05-milestone-5.md`).
- Last accepted tag: `m4-accepted`

## Done

- Session 0: repository setup, tagged `setup-accepted` on commit 21f9b43.
- Milestone 0: the six showdown details a to f confirmed (decisions D8 to D13).
- Milestone 1: core engine. Merged into `main` on 2026-11-17 and tagged `m1-accepted`. Branch `m1-core-engine` kept for history. 11 slice commits plus one post-playthrough fix (C3): plan + cards/hands/seats + meld solver + action/event/observation types + RuleSet/Chance/conformance + StandardConga playing phase + showdown + scoring with −10 and −50 bonuses + event log and exact replay + debug CLI, then the starter-dealt-8 clarification caught during the playthrough. 84 tests pass (fmt, clippy -D warnings, 300-game replay invariant). Rule clarifications C1, C2, C3 encoded in the engine and logged in `docs/design.md`; decisions D14, D15, D16, D17 logged in `docs/decisions.md`.
- Milestone 2: game server, protocol, in-memory transport, TUI. Merged into `main` on 2026-11-18 and tagged `m2-accepted`. Branch `m2-server-tui` kept for history. 12 slice commits: serde on protocol-visible types + `conga-protocol` crate + `conga-server` Transport + MatchState / JSON-lines log / hand orchestrator + `conga-ai` crate with `Player` trait and `RandomPlayer` + `conga-tui` terminal client + `conga-cli` play/replay subcommands, then a TUI redesign (slices 8–12) replacing the numbered action list with card-tile rendering, a live cursor, meld-cycle mode and manual card reorder. 220 tests pass. Decisions D18–D25 logged in `docs/decisions.md`.
- Milestone 3: baseline agents + evaluation harness. Merged into `main` on 2026-11-19 and tagged `m3-accepted`. Branch `m3-baselines-eval` kept for history. 7 slice commits plus one lockfile cleanup: `Player` sync + `GreedyPlayer` (D27) + `RuleBasedPlayer` (D28) + headless runner with paired deals (D29) + `EvalReport` with Wilson CI and `conga-cli eval` + stalemate terminal `HandTerminal { Closed, Stalemate }` (D31, C4) + 2000-game `#[ignore]`d regression. The planned `rule_based_beats_greedy` assertion was dropped: three seed smokes showed D28 loses to greedy at 36–38%, so the shipped regression asserts `rule_based_beats_random` (100%) instead. 261 tests pass (263 with `--ignored`). Decisions D26–D31 logged in `docs/decisions.md`; rule clarification C4 in `docs/design.md`.
- Milestone 4: belief tracker + (scaffolding) search agent. Merged into `main` on 2026-11-20 and tagged `m4-accepted`. Branch `m4-belief-search` kept for history. 5 slice commits plus 1 pivot + 1 diagnostic-knobs commit + 5 progress updates: `StandardConga::sample_state` + conformance test 5 (D34) + `BeliefTracker` module (D32) + `SearchPlayer` PIMC (D33, D36) + `conga-cli eval/play --samples/--ai` flags + rule-based v2 refactor onto BeliefTracker (D38). The standalone search cannot beat greedy (three diagnostic evals: 4% default, 0% random rollouts, 12.5% opp_weight 0.3) because of structural strategy fusion — rollout policy is stronger than root policy; D37 defers the standalone search to m7 with the learned value model. The m4 exit test ("search beats rule-based") is pushed with it; the belief tracker + conformance test 5 + m3 regressions carry the milestone. 281 tests pass (288 with `--ignored`). Decisions D32–D38 logged in `docs/decisions.md`.

## Open questions for Simone

None.

## Deviations from the design doc

- D14: `RuleSet::sample_state` signature uses `&mut dyn Rng` (rand_core 0.10 deprecated `RngCore`). Design doc updated in the same commit.
- D31: A new slice 6 (stalemate terminal) was inserted into the m3 plan ahead of the originally-numbered slice 6; exit-test regression is now slice 7. Reason: slice 5 playtesting surfaced an infinite-loop in two deterministic agents that required an engine-level fix before the regression could run.
- Slice 7 regression replaced `rule_based_beats_greedy` with `rule_based_beats_random`. Three seed smokes (200 games each) showed rule-based consistently losing to greedy at 36–38%; the D28 safety heuristic underperforms plain greedy against a deterministic opponent. Shipping with the truthful regression; D28 rework is deferred.
- D37: The m4 design-doc exit test ("search beats rule-based") was not met and is pushed to m7. Three diagnostic evals showed PIMC with greedy rollouts cannot beat greedy (0–12.5%) due to structural strategy fusion (rollout policy stronger than root policy). The fix needs a non-greedy rollout leaf, which is m7's learned value model. `SearchPlayer` ships as scaffolding (beats random 100-0).
- D38: Rule-based v2 missed the D35 gate (34–37% vs greedy on three 200-game smokes, same as D28's 36–38%). Kept as a refactor onto the belief tracker (same strength, cleaner code) rather than reverted. A rule-based that actually beats greedy is deferred to m5 where match-equity gives a stronger signal than the reactive-safety heuristic can.

## Next

- Milestone 5: match equity and score-aware play. Kickoff prompt at `docs/prompts/05-milestone-5.md`. The m5 agent is expected to be the first non-scaffolding ladder-climber — belief tracker + match equity should be enough to beat greedy without needing the m7 value model.
