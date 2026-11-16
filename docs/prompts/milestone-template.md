# Milestone kickoff template

Use this for milestones 2 to 9. Fill in the three placeholders, start a new Claude Code session in plan mode and paste the text below the line.

- `<N>`: the step number in the Milestones table of `docs/design.md`
- `<deliverable>`: the Deliverable cell of that row
- `<exit test>`: the Exit test cell of that row

---

We are starting milestone <N> from `docs/design.md`: <deliverable>.

## Plan, with no code yet

1. Read `docs/progress.md`, `docs/decisions.md` and the parts of `docs/design.md` this milestone touches.
2. Confirm that `main` holds the tag of the previous milestone and that the checks pass. Report anything that is off.
3. List what you do not know yet: unclear rules, gaps in the design and library APIs you have not verified. Research what you can and name each source. Bring the rest to me as questions with options.
4. Propose the slices. For each one give the goal, the files, the tests, the check that proves it and how I can try it by hand.
5. Wait for my approval. Then save the plan to `docs/plans/m<N>.md`, create the milestone branch and start slice 1.

## Exit test

<exit test>

Follow the working loop in `CLAUDE.md`: one slice, checks, commit, stop report, then wait for me.

---

# Short prompts for the loop

**Refine a slice**

> Refine slice <k>: <what to change>. Stay inside the slice. Run the checks, commit and give me a short stop report.

**I added tests**

> I added tests in <file>. Run them. If one fails, tell me whether the code or the test is wrong before you change anything.

**Continue**

> Slice <k> is accepted. Start slice <k+1> from `docs/plans/m<N>.md`.

**Accept a milestone**

> Milestone <N> is accepted. Merge the branch into `main`, tag `m<N>-accepted` and update `docs/progress.md`. List anything in `docs/design.md` that should change after this milestone, and wait for my answer before you edit it.

**Resume after a break**

> Read `docs/progress.md` and `docs/plans/m<N>.md`. Tell me where we are and what the next step is. Write no code yet.

**Context is getting long**

> Update `docs/progress.md` with everything a new session needs to continue. Then stop, and I will start a fresh session.
