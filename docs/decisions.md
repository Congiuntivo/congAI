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

## Open

| Id | Question | Status |
| --- | --- | --- |
| O1 | Showdown details a to f in `docs/design.md` | Proposals written, waiting for Simone's answer |
