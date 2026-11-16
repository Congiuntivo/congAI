# Conga AI

A Rust engine and AI for Conga, the Uruguayan card game. The goal is to explain the AI's decisions and find tactics a person can use. Simone owns the project and reviews every step.

## Read first

- `docs/design.md` is the source of truth for rules, architecture, stack and milestones. Read the sections a task touches.
- `docs/decisions.md` records every decision made after the design doc.
- Current state: @docs/progress.md

## Working loop

Work runs in the milestones of `docs/design.md`, cut into slices. One slice is one small change that can be tested.

1. **Plan.** At the start of a milestone, stay in plan mode. Propose the slices, the test for each and your open questions. Wait for approval, then save the plan to `docs/plans/m<N>.md`.
2. **Build one slice.** Write the tests with the code. Stay inside the slice.
3. **Check.** Run the three commands under Checks. Fix every failure before you stop.
4. **Commit.** Commit on the milestone branch once the checks pass.
5. **Stop and report.** Write a stop report in the format below, update `docs/progress.md` and wait. Simone reads the code, may add tests and may run it.
6. **Refine or continue.** Apply the changes Simone asks for, or start the next slice once Simone says so.
7. **Accept.** When Simone accepts the milestone, merge the branch into `main`, tag `m<N>-accepted` and update `docs/progress.md`.

Never start a new slice or a new milestone without Simone's go-ahead.

## When to stop early

Stop before writing code in each of these cases.

- **Missing information.** You are unsure how a library, a tool or a rule behaves. Research first: official docs, docs.rs, the crate source or a small experiment. Report what you found and where. Never guess an API from memory.
- **Design choice.** The design doc is silent, ambiguous or looks wrong. Present two or three options with their trade-offs and your recommendation. Wait for the answer, then log it in `docs/decisions.md`.
- **Deviation.** The code needs to differ from `docs/design.md`. Ask first. After approval, update the design doc in the same commit.
- **New dependency.** A crate is missing from the workspace table in the design doc. Ask first, and give its current version and maintenance status.
- **Scope.** A task would touch a later milestone. Leave it and note it under Next in `docs/progress.md`.

## Checks

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Git

- One branch per milestone, created from `main`: `m<N>-<short-name>`.
- Small commits, each with passing checks. Message format: `m<N>: <what changed>`.
- Stage only the files of the slice.
- Never push, force, rebase or rewrite history unless Simone asks.

## Rules of the codebase

- `conga-core` does no input or output, reads no clock and uses no unseeded randomness.
- Every rule lives behind the `RuleSet` trait. No crate outside `conga-rules` hard-codes a rule of the variant.
- `conga-ai` works from observations and from states it samples itself. It never receives the real state.
- A game is stored as rule set id, every shuffle result and the action list. A replay must reproduce every state.
- Library code uses no `unsafe` and no `unwrap` without Simone's approval. Tests may use `unwrap`.
- Tests written by Simone are part of the specification. Never delete or weaken one. If one fails, fix the code or ask.

## Stop report

```
## Stop report: m<N>, slice <k>: <name>
Changed: <file> - <one line>, one per file
Try it: <commands to run or play>
Checks: fmt, clippy, tests (<count> passed)
Tests added: <what each group covers>
Not covered yet: <gaps Simone may want to test>
Questions: <decisions needed, or none>
Next slice: <proposal>
```
