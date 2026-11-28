# Milestone 7: value model

Start a new Claude Code session in plan mode. Paste the text below the line as the first message.

---

We are starting milestone 7 from `docs/design.md`: a learned value model that evaluates an observation, is callable from the Rust search as a rollout leaf, and reaches *equal strength at lower cost per decision, or higher strength at equal cost* than the current ladder top. This milestone also discharges the m4 design-doc exit test ("search beats rule-based"), which was deferred to m7 under D37 because PIMC-with-greedy-rollouts cannot beat greedy.

Milestone 6 is accepted (tag `m6-accepted` on `main`). What's in place:
- Core engine, `RuleSet` trait, `StandardConga`, game server, protocol, TUI, headless eval harness with Wilson CI.
- Seven baseline agents dispatched via `BaselineAgent`: `random`, `greedy`, `rule-based`, `search` (scaffolding, D37), `score-aware` (m5, D45), **`clean-split-close`** (m6, D50 — the close-card picker factored out of score-aware).
- `BeliefTracker` (D32) with `notify` + `known_held` + `sample_state`.
- 2-player match equity table (161×161 at 1-pt resolution, dealer-agnostic, committed under `crates/conga-ai/assets/equity_2p.json` as N=50 greedy-vs-greedy per cell).
- m6 toolchain: `conga-ai::features` (D48), `conga-ai::disagreement` + `mine-disagreements` CLI (D46, committed `m6_disagreements.csv`), `conga-ai::counterfactual` + `counterfactual-sweep` CLI (D46, committed `m6_counterfactuals.json`), `conga-ai::validation::validate_tactic` + `validate-tactic` CLI (D49), `conga-ai::candidates::CleanSplitCloseWrapper` (D47, D50).
- `#[ignore]`d exit regressions: `greedy_beats_random`, `rule_based_beats_random`, `score_aware_beats_rule_based`, `clean_split_close_beats_rule_based`.
- Rule clarifications C1-C4 encoded. Decisions D1-D50 logged.

Important m6 outcomes that shape m7:
- **D50**: `CleanSplitCloseWrapper<RuleBased>` passed vs rule-based at 57.3% (Wilson LB 0.5421), but landed **statistically 50/50 vs score-aware**. The close-card picker captures essentially the full score-aware edge, which means score-aware's "decline to close" equity gate (D41) adds negligible marginal win rate. The equity-table-only evaluator is near its ceiling at Close decisions. **Non-Close decisions (Draw, Discard) are where a learned value head should pay for itself.** The design-doc rollout-leaf role for the value model aligns exactly with this gap.
- The `conga-ai::features` surface is bounded (10 fields: unmatched_total, unmatched_count, meld_count, can_close, close_improver_count, own/opp distance to elim, top_discard, turn_number, known_opp_cards_count). It's a vocabulary, not a tensor encoding. m7 will likely need a richer *fixed-size seat-relative* encoding — propose it, then commit to a schema.
- `SearchPlayer` is still scaffolding. m7 is both its proving ground (give it a non-greedy leaf) and its exit test.

## Before the plan

Decisions to settle before slicing. Ask me each and record the answers in `docs/decisions.md`.

1. **Where training lives.** The design doc covers two choices:
   - **Training in Python, inference in Rust via a hand-written forward pass** (design-doc "current lean" at `docs/design.md` line 349, choice 2). Python owns the self-play orchestration, data pipeline, PyTorch loop. Rust loads the trained weights as JSON/npz and runs a hand-written matmul. Small network cost: a few dozen lines of forward-pass Rust. Lets WebAssembly inherit inference for free later.
   - **Training in Rust with `burn` or similar.** One language, one codebase. Longer slog to train anything, and `burn`'s maturity story is a research question before we commit.
   - **Training in Rust by hand** (gradient descent on a small MLP). Simplest dep-wise; slowest to iterate.
   Propose one. The pick cascades through the rest of the plan (what data format, what crates, which tests).

2. **Network architecture, scoped small.** Propose a concrete first architecture — input dimension, hidden layers, output head(s). Two output-head options:
   - **Value-only.** Scalar match-equity prediction from an observation + sampled belief. Replaces greedy rollouts at the search leaf.
   - **Policy + value (AlphaZero-style).** Action distribution prior + value scalar. More headroom but more moving parts and a training-time loss-mixing decision.
   Lean small: 2-3 hidden layers, <1 MB of weights. Enough to see signal; enough that a hand-written forward pass is honestly a few dozen lines.

3. **Input encoding.** The design doc requires "seat-relative, one fixed size for 2 to 4 players" (line 378). The `conga-ai::features` struct is a feature *vocabulary*, not a tensor. Propose the fixed-size encoding — plausibly concatenation of (own-hand bitmask or one-hot, discard-history position encoding, scores one-hot, hand sizes, belief `known_held` bitmasks, phase one-hot). What's the fixed size for 2-player? For 4? The design-doc constraint is "same encoding size for 2 to 4 players" so the per-seat slots must be masked out when seats are absent.

4. **Self-play data source and opponent pool.** The design doc at line 332 leans on PPO self-play with strong entropy regularization. Concrete questions:
   - Do we generate labelled (obs, outcome) data from paired matches among the current ladder (score-aware vs score-aware, clean-split-close vs rule-based, etc.) and train a supervised value head? Or do we start with PPO/self-play against a frozen pool and train policy+value jointly?
   - What's the label at a non-terminal observation? Match outcome (win=1/loss=0) propagated back? Hand score delta? Or the equity-table lookup at hand end?
   Propose a concrete first dataset (N games, from which agents, labelled how). Keep it in reach — the first value model doesn't need to be strong, it needs to give the search a non-greedy leaf (D37's unmet need).

5. **Search integration.** How does the trained model plug into `SearchPlayer`? Three options:
   - **Rollout leaf.** PIMC still plays rollouts, but at depth D the rollout cuts off and the value head evaluates the leaf. Small change to `SearchPlayer` (D33's PIMC-with-shared-determinization stays; a new `RolloutPolicy` variant reads the model).
   - **One-ply with value head.** Drop rollouts entirely; score every action by `belief-sampled value head evaluation`. Faster per decision; more model dependence.
   - **Both, as separate agents.** `SearchPlayer` still does rollout, a new `ValueOnlyPlayer` does one-ply.
   Propose one as first. The m4 scaffolding `SearchPlayer` is already there to inherit.

6. **Exit-test bar.** The design-doc m7 exit test is "equal strength at lower cost per decision, or higher strength at equal cost". Concretise:
   - "Higher strength at equal cost" against *which* baseline? Score-aware is the obvious target; clean-split-close has essentially the same strength. A clean win is Wilson LB > 0.5 vs score-aware (and vs clean-split-close) at 500+ paired deals.
   - "Equal strength at lower cost" means fewer belief samples or fewer rollout steps for the same win rate. Propose a cost metric (decisions/sec, or samples per decision) and a parity threshold.
   - Also discharge the D37-deferred "search beats rule-based" — a value-model-backed search should easily clear that Wilson gate as a lower-bound check.

7. **Where the model and training lives on disk.** Follow the m5 precedent: a committed artefact under `crates/conga-ai/assets/` (next to `equity_2p.json`) for the weights. If training is in Python, a sibling path such as `training/` or `python/` for the training scripts, with a reproducible pipeline (seed-fixed, logged) so the committed weights can be regenerated. If training is in Rust, propose where the training binary lives.

## Plan, with no code yet

1. Read `docs/progress.md`, `docs/decisions.md` (D1-D50), `docs/design.md` (sections "Stack and tooling" + "The boundary between Rust and Python" + "AI plan" + "Building blocks 4-6" + the Milestone 7 row + "Choices still open for discussion" row 2 + "design-matters" rows on Objective, Opponents, Observations and the Value model architecture row of the "wait" table), `docs/plans/m6.md` + `docs/plans/m5.md` for the shape of a milestone plan, and `crates/conga-ai/src/{search,features,belief,equity,score_aware,candidates/clean_split_close}.rs` to see what plumbing already exists.
2. Confirm that `main` holds `m6-accepted` and that the three CLAUDE.md checks pass. Report anything that is off.
3. List what you do not know yet: whether a hand-written forward pass on a 2-3-layer MLP is ergonomic enough to maintain in Rust alongside a Python training loop; whether `burn` or `candle` or `ort` are mature enough to be considered; what label the value head should regress on (win probability from equity-table lookup at hand end? direct match outcome? hand-score residual?); how the input encoding interacts with the belief tracker (feed a single sampled hidden state per inference? or features that marginalise over K samples upstream?). Research what you can and bring the rest to me as questions with options.
4. Propose the slices. For each one give the goal, the files, the tests, the check that proves it and how I can try it by hand. At least one slice must be *end-to-end minimal* (a tiny untrained model with hand-coded weights wired into `SearchPlayer`, so the integration path is proven before any training pipeline exists). The last slice runs the exit regression end-to-end on a trained model.
5. Wait for my approval. Then save the plan to `docs/plans/m7.md`, create the milestone branch (propose a name, e.g. `m7-value-model`) and start slice 1.

## What the plan must cover

Starting point. Propose a better cut if you see one.

- A fixed-size seat-relative observation encoding, `encode(obs, belief) -> Vec<f32>` (or `[f32; N]`), in `conga-ai` or a new `conga-ai::encoding` module. Documented size and layout, same for 2 and 4 players.
- A `ValueModel` abstraction in `conga-ai` that takes an encoded observation and returns a scalar (and optionally a policy prior). First impl is a hand-written MLP forward pass reading JSON-stored weights. Pin the file path next to `equity_2p.json`.
- A rollout-leaf integration in `SearchPlayer` so the existing PIMC uses the model instead of greedy at the leaf depth. Keep the greedy-leaf path behind a flag so we can A/B.
- A training pipeline, scoped to the smallest honest shape: self-play generation, encoding, label computation, training loop, weight export to the committed file format. Not shipped as a "release-quality trainer" — shipped as a reproducible script that produces the committed weights from a seed.
- Exit-test regressions: `search_with_value_model_beats_rule_based` (discharges D37), and a comparison test documenting cost-per-decision or win rate at equal cost against score-aware / clean-split-close.
- No change to `RuleSet` or `Player::act`. The value model is a new type that `SearchPlayer` owns.
- No speculative scope: 3+ player support and PPO policy-training are out of this milestone unless they fall out of the value-only architecture for free. The design doc's PPO discussion is a tool choice; it doesn't need to be the m7 implementation.

## Exit test

The search agent, with the value model as its rollout leaf, clears both:
1. **Lower bound (D37 discharge)**: beats rule-based outside the 95% Wilson CI on 1000 paired deals (`#[ignore]`d regression in `crates/conga-ai/tests/exit_regressions.rs`).
2. **Design-doc target**: *either* higher strength at equal cost (Wilson LB > 0.5 vs score-aware at the same K per decision), *or* equal strength at lower cost (same win rate vs score-aware at fewer samples per decision), documented in `docs/experiments/m7/`.

If only (1) passes and (2) misses, that is a clean partial result worth shipping with a defer-note for m8: the value model works, the strength-per-cost crossover needs more training. The validation loop and the diagnostic tools built in m6 (disagreement mining, counterfactual probes) should be the lens through which "why did (2) miss?" is answered.

Follow the working loop in `CLAUDE.md`: one slice, checks, commit, stop report, then wait for me.
