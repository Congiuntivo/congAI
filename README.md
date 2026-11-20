# Conga AI

A Rust engine and AI for Conga, the Uruguayan card game. The goal is an agent strong enough to explain its own decisions and surface tactics a human player can use at the table.

Conga is an imperfect-information game. A player sees their hand, the discard history and the public melds; opponents' hands are hidden. The agent has to reason about what each opponent could hold, and the rest of the design answers to that constraint.

## Status

Milestones 1 to 3 are accepted. Milestone 4 (belief tracker, search agent and a rule-based v2) is in progress with three of four slices committed:

- Slice 1: `StandardConga::sample_state` with conformance test 5 (every sampled state reproduces the observation).
- Slice 2: `BeliefTracker` over opponent hands, updated on every public event (draws from stock vs discard, discards, melds laid and adds to melds).
- Slice 3: `SearchPlayer` (PIMC with shared determinization) behind the shared `Player` trait.

`cargo test --workspace` passes with 281 tests. See [`docs/progress.md`](docs/progress.md) for the active slice and [`docs/design.md`](docs/design.md) for the full roadmap.

## Try it

Rust 1.99+, edition 2024.

```
cargo test --workspace
cargo run -p conga-cli -- play --seed 42 --players 2 --ai search --log /tmp/match.jsonl
cargo run -p conga-cli -- replay /tmp/match.jsonl
cargo run -p conga-cli -- eval --p1 search --p2 greedy --games 500
```

`play` opens a 2-player match in the terminal against a baseline agent selectable with `--ai` (`random`, `greedy`, `rule-based`, `search`). Arrow keys move the cursor between stock, discard and your hand tiles; Enter acts on the hovered tile; `c` closes, `p` passes, `m` and `a` enter meld / add-to-meld cycle modes; Shift+arrows reorder cards in the hand. `--suits spanish` switches the glyph theme. `replay` verifies every hand in a log replays to the same `HandResult`. `eval` runs paired deals and prints the win rate with a Wilson confidence interval.

## Design notes

The engine does no I/O, reads no clock and uses no unseeded randomness. A game is stored as a ruleset id, every shuffle result and the action list; the exact-replay property is checked by a conformance test over thousands of random games. Library code uses no `unsafe` and no `unwrap` outside tests.

Rules are pluggable through the `RuleSet` trait. `StandardConga` is the first one and lives in `conga-rules`. No crate outside `conga-rules` hard-codes a rule of the variant.

The search agent uses PIMC. It samples a batch of complete states from the belief tracker, solves each determinization with a short rollout and aggregates over the samples. The belief tracker starts from a uniform prior over opponent hands consistent with the deal and tightens on every public event.

## What's next

- Milestone 4 slice 4: a rule-based v2 that consumes the belief tracker's output (such as the probability a card is safe to discard) so the baselines bracket the search agent properly.
- Milestones 5 and 6 (see `docs/design.md`): neural rollout and value heads trained from search play, Python training loop in PyTorch, policy-guided search.
- Then a web client and an explanation layer that turns a search result into a human-readable justification: which belief samples backed the chosen action and which tactical motifs applied.

## Workspace

| Crate | Role |
| --- | --- |
| `conga-core` | Shared types, the `RuleSet` trait, the meld solver, logs, replay. No I/O. |
| `conga-rules` | Rule-set modules. `StandardConga` is the first and currently the only one. |
| `conga-protocol` | Wire messages and a versioned envelope over JSON lines. |
| `conga-server` | Authoritative match loop, `Transport` trait with an in-memory implementation, per-match log writer. |
| `conga-ai` | `Player` trait, baseline agents (random, greedy, rule-based), `BeliefTracker`, PIMC search, headless eval harness. |
| `conga-tui` | Terminal client on ratatui: card tiles, cursor navigation and a meld cycle mode. |
| `conga-cli` | Entry points: `debug-hand`, `play`, `replay`, `eval`. |

## Docs

- [`docs/design.md`](docs/design.md): rules, architecture, stack, milestones.
- [`docs/decisions.md`](docs/decisions.md): every decision taken after the design doc.
- [`docs/progress.md`](docs/progress.md): current milestone, slice and branch.
- [`docs/plans/`](docs/plans): approved slice plan per milestone.
- [`CLAUDE.md`](CLAUDE.md): the slice-by-slice loop I follow with the AI pair.

## Development

Three checks pass before every commit on a milestone branch:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## License

MIT. See [`LICENSE`](LICENSE).
