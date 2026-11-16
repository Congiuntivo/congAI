# Progress

Claude updates this file at every stop. A new session reads it first.

## Now

- Milestone: 1 (core engine), in progress
- Slice: 6 — showdown phase. Code and tests done, awaiting Simone's review. Slices 1-5 accepted.
- Branch: `m1-core-engine`
- Last accepted tag: `setup-accepted`

## Done

- Session 0: repository setup. Three commits on `main`: initial contents and .gitignore, cargo workspace scaffold with `conga-core`, `conga-rules`, `conga-cli` each with one smoke test, and the two export-leftover fixes in `docs/design.md` plus this progress update. All three CLAUDE.md checks pass (fmt, clippy, 3 tests). Tagged `setup-accepted` on commit 21f9b43.
- Milestone 0 is complete: the six showdown details a to f are now confirmed (decisions D8 to D13).
- Milestone 1 plan saved to `docs/plans/m1.md` and branch `m1-core-engine` created from `main`.

## Open questions for Simone

None.

## Deviations from the design doc

None.

## Next

- Milestone 1: write the plan, get Simone's approval, then start slice 1 on branch `m1-core-engine`.
