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
| D15 | 2026-11-16 | Rule-6 clarification C1: `Close(c)` with the card just taken from the top of the discard pile is legal. Rule 6 applies only to face-up re-discards. Logged as C1 under "Rule clarifications" in `docs/design.md` | Rule 6's stated purpose is to prevent a free pass; closing is not a free pass (it ends the hand) so the ban does not extend to it. Keeps the move available |
| D16 | 2026-11-16 | Detail-(c) clarification C2: the closer's `Pass` in their first showdown turn is legal only when their hand IS the closing residual (≤ 2 cards summing ≤ 5). Must physically lay down melds. Logged as C2 under "Rule clarifications" in `docs/design.md` | Matches "lay down a split" in detail (c) and the physical game: opponents verify by looking at the table. Permissive reading would let the closer hide their melds and deny opponents meld-add targets |
| D17 | 2026-11-17 | Starter-deal clarification C3: the starter is dealt one extra card, the discard pile begins empty, and the starter's first action is a `Discard` (no draw). Logged as C3 under "Rule clarifications" in `docs/design.md`. `StandardConga::new_hand` updated and the dependent slice-5 tests reshaped | How Simone's table plays it; keeps the extra card private rather than revealing one face-up at the start |
| D18 | 2026-11-17 | Wire and log format: JSON lines (one JSON object per line, UTF-8, `\n`-terminated) | Confirms design-doc choice 3. Logs stay readable, same format for wire and storage, identical path later over WebSocket binary frames |
| D19 | 2026-11-17 | The random agent lives in a new minimal `conga-ai` crate, containing only the `Player` trait and `RandomPlayer` in m2. The full baseline suite (greedy, rule-based, evaluation harness) is added in m3 | Keeps AI code out of `conga-server`. m3 fills the crate without moving files, and the server stays orchestration-only |
| D20 | 2026-11-17 | TUI v1 scope: single human seat vs single random-AI seat. No hand-over screen, no seat selector | Matches the m2 exit test exactly. Local multiplayer with hand-over is pulled in when a later milestone needs it |
| D21 | 2026-11-17 | `MatchState` (scores across hands, dealer rotation, elimination tracking, match status) lives in `conga-server`, not in `conga-rules`. `conga-rules` stays single-hand | Match-level orchestration is not a rule; it belongs to the server even though rules 12 and 16 drive its decisions. Keeps `conga-rules` focused on the per-hand engine |
| D22 | 2026-11-17 | `conga-cli replay <log-file>` reads a JSON-lines match log, prints each hand's state sequence and the final match status. Reuses the slice-8 hand `replay` helper lifted to match level | Confirms the m2 prompt's proposal; no `--seat` or `--step` flag for now, those can land in a later milestone if needed |

| D23 | 2026-11-18 | TUI v2 uses a card-cursor model: stock / top discard / each hand card is a navigable tile (4-row ASCII box). `←/→` move the cursor, `Enter` acts on the current tile, `d` discards the selected card, `c` closes with it, `p` passes, `m` enters lay-meld cycle, `a` enters add-to-meld cycle, `Shift+←/→` reorders cards in the hand, `q` quits. The numbered 1..0 action list is removed | The numbered list is scriptable but not playable. The card-cursor model keeps every action physically tied to a card the player sees, matches how one plays with physical cards, and scales to multi-card meld selection without a submenu |
| D24 | 2026-11-18 | Internal deck stays `C/D/H/S` in `conga-core`. The TUI renders cards with a user-visible suit theme chosen via `conga-cli play --suits=french\|spanish` (default `french`). French uses `♣ ♦ ♥ ♠`; Spanish uses `✦ ● ♡ ✚` (bastos / oros / copas / espadas). The log format and the `replay` CLI output stay as the raw `"7C"` serialisation | Keeps the engine, log and protocol unchanged. Theme is a display-only concern; swapping it needs no engine, protocol or log changes. Single-cell glyphs keep the card-tile layout aligned in every terminal |
| D25 | 2026-11-18 | Meld builder uses a cycle model: pressing `m` enters a mode that iterates the already-legal `LayMeld` actions with `←/→`, `Enter` commits, `Esc` cancels. Same model for `a` + `AddToMeld`. No multi-select + validate | Simpler: reuses `legal_actions` directly, no selection-to-meld reconciliation. Cheaper to test and implement. Loses nothing, since the meld solver already enumerates every legal meld the player could want to lay |

## Open

None.
