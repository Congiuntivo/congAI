# Decisions

Every decision taken after or outside `docs/design.md`, oldest first. Add a row whenever Simone answers a design question.

| Id | Date | Decision | Reason |
| --- | --- | --- | --- |
| D1 | 2026-11-13 | The rule set in `docs/design.md` is confirmed, except the six showdown details | Simone's answers to the rule questions |
| D2 | 2026-11-15 | Rust for the engine, server, TUI and search. Python for training and analysis | Rollout speed, one engine for every client and Simone's preference |
| D3 | 2026-11-15 | The search is written in Rust from the start | Search quality grows with rollouts per second |
| D4 | 2026-11-15 | Rules are pluggable through the `RuleSet` trait | A rule change should need only a new rule set |
| D5 | 2026-11-15 | Work runs in milestone slices, with a stop for review after each slice | Simone checks each step before the next one starts |
| D6 | 2026-11-15 | One branch per milestone, small commits and a tag on acceptance | Clean points to roll back to |
| D7 | 2026-11-15 | Milestone 1 includes a debug command that plays a hand in the terminal | The rules can be checked by hand before the TUI exists |
| D8 | 2026-11-16 | Showdown detail a: the closer can add cards to other players' melds in later showdown rounds | Simone confirmed the proposal |
| D9 | 2026-11-16 | Showdown detail b: a closer who empties their hand during the showdown does not score minus 10. The bonus needs 0 unmatched cards at the moment of closing | Simone confirmed the proposal |
| D10 | 2026-11-16 | Showdown detail c: the closer must lay down a split that meets the closing condition, and the server checks it | Simone confirmed the proposal |
| D11 | 2026-11-16 | Showdown detail d: a player may keep a meld in hand. The kept cards count as points | Simone confirmed the proposal |
| D12 | 2026-11-16 | Showdown detail e: the minus 10 and minus 50 bonuses belong to the closer only | Simone confirmed the proposal |
| D13 | 2026-11-16 | Showdown detail f: hands are hidden during the showdown. Only cards on the table are public until the count | Simone confirmed the proposal |
| D14 | 2026-11-16 | `RuleSet::sample_state` takes `&mut dyn Rng` not `&mut dyn RngCore`. Design doc updated in the same commit | rand_core 0.10 deprecated `RngCore` in favour of `Rng`; using the deprecated trait fails the clippy `-D warnings` check |

## Open

None.
