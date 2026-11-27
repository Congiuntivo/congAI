# m6 candidate: clean-split close-card picker — result

Status: **passed** (2026-11-27). Promoted to `BaselineAgent::CleanSplitClose` and `--ai clean-split-close` (D50).

## Setup

- **Tactic:** `CleanSplitCloseWrapper<RuleBasedPlayer>` from `conga-ai::candidates::clean_split_close`. When the inner (rule-based) picks `Close(_)`, the wrapper enumerates every legal Close option, draws K belief samples once (shared across options for variance reduction), and picks the close-card with the highest post-close equity. Never declines to close.
- **Unchanged baseline:** `RuleBasedPlayer` (same rule-based the tactic wraps). Any delta is attributable to the close-card picker alone.
- **Strong baseline:** `ScoreAwarePlayer` (D41/D45) — rule-based + the same close-card picker + an equity-gated "decline to close" path.
- **Harness:** `conga-ai::validation::validate_tactic`, 500 paired deals per gate (1000 matches), same seeds across both gates for apples-to-apples. Belief samples K=20 (`DEFAULT_CLEAN_SPLIT_SAMPLES`).
- **Seed:** 0x0 base. Same equity table (`crates/conga-ai/assets/equity_2p.json`, N=50 greedy-vs-greedy, D44) as score-aware.

Reproduce:

```
cargo run --release -p conga-cli -- validate-tactic --candidate clean-split-close --pairs 500 --seed 0 --samples 20
```

## Results

### vs unchanged rule-based (pass/fail gate)

| Metric | Value |
| --- | --- |
| Matches | 1000 |
| Tactic wins | 573 |
| Unchanged wins | 427 |
| Win rate | 57.30% |
| Wilson 95% CI | [54.21%, 60.33%] |
| **Wilson LB** | **0.5421** |
| **Pass gate (> 0.5)** | **yes** |
| Mean tactic score | 82.72 |
| Mean unchanged score | 90.96 |
| Hands / match | 13.95 |

### vs strong score-aware (informational)

| Metric | Value |
| --- | --- |
| Matches | 1000 |
| Tactic wins | 511 |
| Score-aware wins | 489 |
| Win rate | 51.10% |
| Wilson 95% CI | [48.00%, 54.19%] |
| Mean tactic score | 85.67 |
| Mean score-aware score | 89.66 |
| Hands / match | 15.10 |

Total wall clock (both gates): 234.3 s on release, 12 cores idle.

## Interpretation

The tactic clears the gate decisively: a 7.3 pp win rate over the unchanged rule-based is slightly above D45's observed gap of ~7 pp for the *full* score-aware agent. The vs-strong gate is statistically indistinguishable from 50/50 (CI straddles 0.5), which means the clean-split close picker captures essentially the full edge that score-aware has over rule-based.

**Design-doc question answered.** The showdown question "which split of a hand leaves the fewest openings?" has a crisp operational answer: when closing, pick the close-card whose discard leaves the cleanest meld split. The -10 clean-split bonus dominates: the equity-table lookup is only needed as the ranking function; the signal is already strong enough that the wrapper performs within noise of the full score-aware agent.

**What this means for the equity-gated decline.** D45 called the decline-to-close path "a no-op in practice." This experiment confirms it quantitatively — removing that path and keeping only the close-card picker gives the same win rate against rule-based as the full score-aware agent (within sampling noise). The slice-3 counterfactual diagnostic also saw declines firing on only 2 of 10 mined positions.

**Mean-score note.** The tactic's mean final score against rule-based (82.72) is 8+ points lower than rule-based's (90.96). That matches the shape of a tactic that lowers *own* scores on close hands (via the -10 bonus) and nudges wins through accumulated score advantage.

## Decision

Promote to `BaselineAgent::CleanSplitClose` and `--ai clean-split-close`. The wrapper takes K=20 samples by default and seeds ChaCha8Rng from the ladder's seed, matching the score-aware pattern.

## Loop demonstrably works

The m6 exit test in the design doc is "the first tactic passes the loop on fresh seeds". The slice-5 regression `clean_split_close_beats_rule_based` runs on 1000 fresh matches at seed 0x1 (unused by prior regressions) and passes. Slice-4's dummy-good / dummy-bad unit tests also exercise the loop at 50 pairs each (Greedy-vs-Random passes; Random-vs-Greedy fails). Loop validated end-to-end.
