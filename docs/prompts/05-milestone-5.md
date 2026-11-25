# Milestone 5: match equity and score-aware play

Start a new Claude Code session in plan mode. Paste the text below the line as the first message.

---

We are starting milestone 5 from `docs/design.md`: a 2-player match equity table plus a score-aware agent that beats its score-blind version over full matches.

Milestone 4 is accepted (tag `m4-accepted` on `main`). The core engine, the `RuleSet` trait, `StandardConga`, the game server, the protocol, the TUI, five baseline agents (`RandomPlayer`, `GreedyPlayer`, `RuleBasedPlayer` v2, `SearchPlayer` scaffolding, `BaselineAgent::Search`), the headless eval harness with Wilson CI, `StandardConga::sample_state` + conformance test 5 and a `BeliefTracker` are all in. Rule clarifications C1-C4 are encoded. Decisions D1-D38 are logged.

Important m4 outcomes that shape m5:
- **D37**: the standalone PIMC search cannot beat greedy because its rollout policy (greedy) is stronger than its root (search itself). The design-doc m4 exit test ("search beats rule-based") is deferred to m7 when the learned value model gives rollouts a non-greedy leaf.
- **D38**: rule-based v2 (belief-tracker-based) is statistically identical to D28 (~35% vs greedy). A rule-based agent that actually beats greedy needs a different *shape* — match-equity-aware rather than reactive-safety-only — which is exactly this milestone.
- `BeliefTracker` is production-quality (D32) and plugs into any agent via `notify` + `known_held` + `sample_state`.

## Before the plan

Decisions to settle before slicing. Ask me each and record the answers in `docs/decisions.md`.

1. **Match equity table representation.** 2-player scores in StandardConga can range roughly from -50 (closer gets Conga bonus with 0 residual) to 110 (just-over-elimination). Propose dimensions, resolution and a floor behaviour for out-of-range scores. Also: is the table (own_score, opp_score, dealer_this_hand) or just (own_score, opp_score)? Dealer flips the starter-dealt-8 asymmetry (C3), which might matter.
2. **How to compute the table.** Monte Carlo: simulate N matches starting from every cell, with both seats playing a fixed baseline (greedy or rule-based) until one is eliminated. Propose N per cell, agent choice and parallelism (rayon? single-threaded?). Rough budget: 161×161 cells × 2 seats × N matches × ~20ms = scales from feasible to not.
3. **Score-aware agent architecture.** Three options:
   - (a) Thin equity layer on top of rule-based: the `Close` decision consults equity ("close iff post-close equity > pass equity"); everything else is rule-based.
   - (b) Full one-ply search: for every legal action evaluate the belief-sampled expected post-hand equity, argmax.
   - (c) Something else you propose.
   Pick one and explain the trade-off.
4. **Belief tracker interaction.** Does the score-aware agent consume the belief tracker (e.g., to sample post-hand residuals)? Or is the first version purely a "what would greedy do, but close/hold based on equity" layer?
5. **Exit test bars.** Concrete numbers for "score-aware beats score-blind": paired-deal count, Wilson CI lower bound. Note: we've seen 35% ± 5% for v2-vs-greedy at 200 games — same precision would want ~400-800 paired deals for a reliable sign.

## Plan, with no code yet

1. Read `docs/progress.md`, `docs/decisions.md` (D1-D38), `docs/design.md` (Building blocks 2/3/5 "Match value", Milestone 5 row, "Evaluation" section, the Objective row of the design-matters table), `docs/plans/m4.md` for the shape of a milestone plan, and `crates/conga-ai/src/belief.rs` + `search.rs` to see what plumbing already exists.
2. Confirm that `main` holds `m4-accepted` and that the three CLAUDE.md checks pass. Report anything that is off.
3. List what you do not know yet: realistic match-equity ranges for StandardConga, the actual distribution of per-hand score changes (do matches typically span 5 hands? 20? the m4 smokes showed 9-14), library APIs you have not verified (serde JSON for equity-table persistence, maybe `rayon` for the first time in this project). Research what you can, bring the rest to me as questions with options.
4. Propose the slices. For each one give the goal, the files, the tests, the check that proves it and how I can try it by hand. Include the "how is the equity table itself stored on disk" step (likely a JSON or CBOR file committed to the repo under `assets/` or similar).
5. Wait for my approval. Then save the plan to `docs/plans/m5.md`, create the milestone branch `m5-match-equity` and start slice 1.

## What the plan must cover

Starting point. Propose a better cut if you see one.

- A 2-player match equity table: types, computation, serialization, loading. Mindful of the design-doc constraint that scores can go negative (closer bonus) and the table needs a floor.
- A score-aware agent per the question-3 architecture. Lives in `conga-ai` next to the existing baselines. Wired through `BaselineAgent` so `conga-cli eval` / `play` can dispatch it.
- A benchmark showing score-aware beats its score-blind version (same base agent without the equity consult) over paired deals.
- No change to `Player::act` or `RuleSet`. The equity table is a module/static in `conga-ai`; agents read it.
- No `rayon` parallelism unless the sequential compute is actually too slow — bring numbers first.

## Exit test

The score-aware agent beats its score-blind version over full matches, outside the 95% Wilson CI. Also: the equity table's own quality is cross-checked — e.g., equity(0, 0) ≈ 0.5, equity(100, 0) ≈ 0 (elimination for own seat), equity(0, 100) ≈ 1 (opponent eliminated). Those invariants should be unit-tested.

Follow the working loop in `CLAUDE.md`: one slice, checks, commit, stop report, then wait for me.
