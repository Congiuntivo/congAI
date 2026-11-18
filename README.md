# Conga AI

A Rust engine and AI for Conga, the Uruguayan card game. The goal is an agent strong enough to explain its own decisions and surface tactics a human player can use.

## Status

Milestones 0, 1 and 2 are accepted: the ruleset, the core engine with the pluggable `RuleSet` trait and the meld solver, and an authoritative server + JSON-lines protocol + ratatui TUI. Milestone 3 (baseline agents and evaluation harness) is in progress. See [`docs/progress.md`](docs/progress.md) for the active slice and [`docs/design.md`](docs/design.md) for the full roadmap.

## Try it

Rust 1.99+, edition 2024 (see the workspace `rust-version` in `Cargo.toml`).

```
cargo test --workspace
cargo run -p conga-cli -- play --seed 42 --players 2 --log /tmp/match.jsonl
cargo run -p conga-cli -- replay /tmp/match.jsonl
```

`play` opens a 2-player match against a random agent in the terminal. Arrow keys move the cursor between stock, discard and your hand tiles; Enter acts on the hovered tile; `c` closes, `p` passes, `m` and `a` enter meld / add-to-meld cycle modes; Shift+arrows reorder cards in the hand. `--suits spanish` switches the glyph theme. `replay` reads the JSON-lines log `play` wrote and verifies every hand replays to the same `HandResult`.

## Workspace

| Crate | Role |
| --- | --- |
| `conga-core` | Shared types, the `RuleSet` trait, the meld solver, logs, replay. No I/O. |
| `conga-rules` | Rule-set modules. `StandardConga` is the first and currently the only one. |
| `conga-protocol` | Wire messages and a versioned envelope over JSON lines. |
| `conga-server` | Authoritative match loop, `Transport` trait with an in-memory implementation, per-match log writer. |
| `conga-ai` | `Player` trait and agents. `RandomPlayer` lives here; greedy and rule-based agents land in m3. |
| `conga-tui` | Terminal client built on ratatui: card tiles, cursor navigation, meld cycle mode. |
| `conga-cli` | Debug helpers and the entry points: `debug-hand`, `play`, `replay`. |

A game is stored as a ruleset id, every shuffle result and the action list. A replay must reproduce every state. `conga-core` does no I/O, reads no clock and uses no unseeded randomness; all rule logic lives behind the `RuleSet` trait.

## Docs

- [`docs/design.md`](docs/design.md) — rules, architecture, stack, milestones.
- [`docs/decisions.md`](docs/decisions.md) — every decision taken after the design doc.
- [`docs/progress.md`](docs/progress.md) — current milestone, slice and branch.
- [`docs/plans/`](docs/plans) — approved slice plan for each milestone.
- [`CLAUDE.md`](CLAUDE.md) — collaboration loop with the AI pair.

## Development

Three checks have to pass before every commit on a milestone branch:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The slice-by-slice workflow is in [`CLAUDE.md`](CLAUDE.md).
