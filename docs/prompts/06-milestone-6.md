# Milestone 6: explanation tools and the validation loop

Start a new Claude Code session in plan mode. Paste the text below the line as the first message.

---

We are starting milestone 6 from `docs/design.md`: the explanation tools that turn the agent's decisions into tactics a human can use, plus the validation loop that proves a candidate tactic is real. Design-doc exit test: *the first tactic passes the loop on fresh seeds.*

Milestone 5 is accepted (tag `m5-accepted` on `main`). What's in place:
- Core engine, `RuleSet` trait, `StandardConga`, game server, protocol, TUI, headless eval harness with Wilson CI.
- Six baseline agents dispatched via `BaselineAgent`: `random`, `greedy`, `rule-based`, `search` (scaffolding — D37), `score-aware` (m5, the first ladder-climber past greedy).
- `BeliefTracker` (D32) with `notify` + `known_held` + `sample_state`.
- 2-player match equity table (D39, D40, D44): N=50 greedy-vs-greedy Monte Carlo, 161×161 at 1-pt resolution, dealer-agnostic, committed as `crates/conga-ai/assets/equity_2p.json`. Loaded via `committed_table()` in `conga-ai::equity`.
- `conga-cli build-equity` subcommand for regeneration (rayon-parallel, D43).
- `#[ignore]`d exit regressions in `crates/conga-ai/tests/exit_regressions.rs`: greedy > random, rule-based > random, score-aware > rule-based.
- Rule clarifications C1-C4 encoded. Decisions D1-D45 logged.

Important m5 outcomes that shape m6:
- **D45**: the score-aware agent's edge over rule-based (~7 pp) comes almost entirely from *picking the close-card that leaves a clean split* (the -10 bonus). The equity-gated "decline to close" path almost never fires on the real N=50 table because the closer's delta is ≤ 0 and the opponent's is ≥ 0 — closing is almost always strictly improving. If m6 wants to find a *tactic* around closing, this is the obvious target to probe.
- The committed equity table is the first table-shaped object in the repo. The validation loop will produce more (candidate tactics, feature distributions, counterfactual sweeps); think about whether `assets/` grows a convention or whether each experiment lives under its own path.
- `SearchPlayer` remains scaffolding. The explanation methods in the design doc ("Per-action values from search", "Value decomposition", "Counterfactual probes") assume a strong agent; score-aware is now the strongest headless option until m7 (value model). Plan m6 around the strongest baseline available *today*.

## Before the plan

Decisions to settle before slicing. Ask me each and record the answers in `docs/decisions.md`.

1. **Which explanation methods first.** The design doc lists seven (per-action values, value decomposition, counterfactual probes, disagreement mining, distillation, targeted experiments, feature attribution). m6 can't ship all. Propose a cut: which two or three to build first, which to defer, and why the cut covers the design-doc "first tactic" exit test.
2. **The first candidate tactic.** The design doc lists seven "first questions for the system" (see `docs/design.md` at the "Explainability and tactic discovery" section). Propose one as m6's concrete tactic target — one where we already suspect an answer (e.g., question 1 about closing thresholds, informed by D45) and where disagreement mining or counterfactual probes should produce a signal. The validation loop needs a definite rule to test.
3. **Feature extraction surface.** The design doc gives a feature vocabulary (own hand, risk, information, context, showdown). Does m6 add a new `conga-ai::features` module that computes these on-demand from `Observation` + `BeliefTracker`, or does each method extract its own as needed? What does the output type look like?
4. **Validation harness.** The loop is "add rule → paired matches against unchanged → paired matches against strong → keep if gain clears CI". Concrete: paired-deal count per gate, which agents play against (score-aware? rule-based? both?), where the results are reported. Can it reuse `run_batch` + `report_from_pairs` as-is or does it need a new thin wrapper?
5. **Where tactic candidates live in code.** Options: (a) each candidate is a feature flag on `RuleBasedPlayer` that `BaselineAgent` can enable; (b) each candidate is its own `Player` impl wrapping a baseline; (c) something else. The choice affects how many agents end up on the ladder.

## Plan, with no code yet

1. Read `docs/progress.md`, `docs/decisions.md` (D1-D45), `docs/design.md` (the "Explainability and tactic discovery" section in full, the Milestone 6 row, the "design-matters" row on Opponents and on Definition of done), `docs/plans/m5.md` for the shape of a milestone plan, and `crates/conga-ai/src/score_aware.rs` + `equity.rs` to see what plumbing already exists.
2. Confirm that `main` holds `m5-accepted` and that the three CLAUDE.md checks pass. Report anything that is off.
3. List what you do not know yet: whether search-based per-action values are feasible on scaffolding `SearchPlayer`, whether the feature vocabulary maps cleanly onto the current `Observation`, whether the first tactic should be tested against score-aware (strongest) or rule-based (baseline). Research what you can, bring the rest to me as questions with options.
4. Propose the slices. For each give the goal, the files, the tests, the check that proves it and how I can try it by hand. At least one slice should produce a *committed* diagnostic artefact (a per-position value dump, a disagreement table, something). The last slice runs the validation loop end-to-end on the chosen candidate tactic.
5. Wait for my approval. Then save the plan to `docs/plans/m6.md`, create the milestone branch `m6-explanations` (or a better name you propose) and start slice 1.

## What the plan must cover

Starting point. Propose a better cut if you see one.

- A feature-extraction surface for `Observation` + `BeliefTracker` state, in the vocabulary of `docs/design.md` "Feature vocabulary".
- At least two explanation methods from the design-doc list, built in `conga-ai` alongside the agents.
- A validation-loop harness that takes a candidate tactic (as a rule or a `Player` wrapper), runs paired matches against an unchanged baseline and against the strong baseline (score-aware), and reports Wilson-CI-clean win-rate gain.
- One concrete candidate tactic (the question-2 answer) run through the loop to passing or failing. If it fails: the loop still works, document the negative result, propose the next candidate.
- No change to `RuleSet`. Feature extraction and explanation code live in `conga-ai`; validation reuses the eval harness.
- No new baseline on the `BaselineAgent` ladder unless the candidate tactic passes the loop. Tactic candidates that fail the gate stay out of the shipped agent list.

## Exit test

The first candidate tactic completes the validation loop — either passing cleanly (Wilson LB > 0.5 improvement over the unchanged baseline on fresh seeds, outside the CI) or failing cleanly (gain within the CI, documented as a negative result with a hypothesis for why). A passing result is the design-doc exit. A clean-failing result at the first candidate is acceptable if the loop itself demonstrably works (e.g., smoke-tested against a hand-built dummy "tactic" that we know is good or bad).

Follow the working loop in `CLAUDE.md`: one slice, checks, commit, stop report, then wait for me.
