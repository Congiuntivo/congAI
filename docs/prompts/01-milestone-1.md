# Milestone 1: core engine

Start a new Claude Code session in plan mode. Paste the text below the line as the first message.

---

We are starting milestone 1 from `docs/design.md`: the core engine with the `RuleSet` trait, the meld solver and my variant as the first rule set.

## Before the plan

The six showdown details, a to f, in the section "Showdown after a close" are proposals that I have not confirmed. Ask me about each one first. Record my answers in `docs/decisions.md` and update the design doc.

## Plan, with no code yet

1. Read `docs/progress.md`, `docs/decisions.md` and these parts of `docs/design.md`: Ruleset, Architecture, Software components and the Milestones row for step 1.
2. List what you do not know yet: unclear rules, gaps in the design and library APIs you have not verified. Research what you can and name each source. Bring the rest to me as questions with options.
3. Propose the slices. For each one give the goal, the files, the tests, the check that proves it and how I can try it by hand.
4. Wait for my approval. Then save the plan to `docs/plans/m1.md`, create the branch `m1-core-engine` and start slice 1.

## What the plan must cover

Treat this list as a starting point. Propose a better cut if you see one.

- Shared types: cards, hands as bit masks, deck, actions, observations and events.
- The meld solver and `evaluate_hand`, tested against hands written out by hand.
- The playing phase: deal, draw, discard, close, the first-turn rule and the empty stock.
- The showdown phase.
- Scoring, bonuses and match status, with elimination above 100.
- `Chance`, the event log and exact replay.
- The conformance tests, generic over `RuleSet`.
- A debug command in `conga-cli` that plays one hand in the terminal from a seed, with all hands visible. I use it to check the rules by hand, above all the showdown.

## Exit test

The conformance tests pass on a large batch of random games, and a logged game replays to identical states. I also play several hands with the debug command and agree with every legal move and every score.

Follow the working loop in `CLAUDE.md`: one slice, checks, commit, stop report, then wait for me.
