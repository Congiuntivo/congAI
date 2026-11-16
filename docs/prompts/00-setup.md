# Session 0: repository setup

Start Claude Code in the project folder, which already holds `CLAUDE.md` and `docs/`. Paste the text below the line as the first message.

---

This session sets up the repository. It writes no game code.

1. Read `CLAUDE.md`, `docs/progress.md` and `docs/decisions.md`. Then read `docs/design.md` once from start to end.
2. Report the toolchain you find: `rustc`, `cargo` and `git`, with versions. Look up the current stable Rust release and edition. Tell me if anything should be installed or updated, and wait for my answer before you install.
3. Run `git init` with `main` as the branch. Add a `.gitignore` for Rust and Python. Make the first commit with the files that already exist.
4. Create the Cargo workspace with only the crates milestone 1 needs: `conga-core`, `conga-rules` and `conga-cli`. Each one gets an empty library or binary and one trivial test. Add `rust-toolchain.toml` with `rustfmt` and `clippy`.
5. The design doc was exported from another tool. Fix two leftovers of the export and show me the diff:
   - One line in the Architecture section holds the words "embedded content: architecture", possibly with escape characters around them. A diagram stood there. Replace the line with a plain-text diagram built from the Components table.
   - The Core interfaces sketch uses Python notation. Add one sentence under it saying that the Rust `RuleSet` trait in the Pluggable rule sets section replaces it for the engine.
6. Run the checks from `CLAUDE.md`, commit and update `docs/progress.md`.
7. End with a stop report. After I accept, tag the commit `setup-accepted`.

Follow `CLAUDE.md` at every step. Ask me whenever something is unclear.
