# Conga AI: design document v0.1

Nov 13, 2026 · @Simone De Carli

## Purpose and scope

The project builds a strong Conga agent in order to explain its decisions and extract tactics a human can use. A playable game is a by-product, and it also serves as the test bench.

The project is finished when it has produced three things:

1. A rules engine with pluggable players (human, AI, remote).
2. An agent that beats rule-based baselines by a measured margin.
3. A list of tactics, each one confirmed by experiment.

| Scope | Items |
| --- | --- |
| Built now | Rules engine, meld solver, agents, evaluation harness, explanation tooling, TUI |
| Designed for, built later | GUI or web client, LAN play, online play |
| Out of scope | Accounts, matchmaking, rankings, mobile apps |

## Ruleset

All 21 rules and the six showdown details are now confirmed.

### Fixed rules

| Rule | Value |
| --- | --- |
| Deck | 40 cards, ranks 1 to 10 in each of four suits |
| Jokers | None |
| Players | 2 to 4 |
| Closing condition | At most 2 unmatched cards, with their values summing to at most 5 |
| First turn | Closing is forbidden |

### What the closing rule implies

With a 7-card hand and melds of at least 3 cards, only three hand shapes can close. The hand size of 7 is confirmed.

| Unmatched cards | Melds needed | Limit on unmatched cards |
| --- | --- | --- |
| 0 | A meld of 3 plus a meld of 4, or one 7-card run | None |
| 1 | Two melds of 3, or one 6-card run | Value 1 to 5 |
| 2 | One 5-card run | Pairs 1+1, 1+2, 1+3, 1+4, 2+2 or 2+3 |

The two-card close needs a 5-card run, so it will be rare. Most closes will come from two melds of 3 plus one low card.

### Confirmed rules

You confirmed these rules on 13 November 2026, and rows 8 to 12 changed from the first proposal. Every one is encoded in a `RuleSet` implementation, and the architecture section shows how to plug in another one.

| # | Question | Rule | What it affects |
| --- | --- | --- | --- |
| 1 | Hand size | 7 cards, 8 after drawing | Which hands can close |
| 2 | Melds | Set: 3 or 4 cards of one rank. Run: 3 or more consecutive cards of one suit. No wrap from 10 to 1. Each card sits in one meld | Meld solver |
| 3 | Card values | Face value, 1 to 10 | Scoring |
| 4 | Turn structure | Draw from the stock or take the top discard, then discard one card. A player closes by placing that discard face down | Action space |
| 5 | Meaning of "first turn" | Each player's own first turn | Hands dealt ready to close |
| 6 | Taking the top discard and discarding the same card | Forbidden | Prevents a free pass |
| 7 | Is closing mandatory when possible | No | The decision to close or to play on |
| 8 | Points at the close | After the showdown each player adds the value of the cards left in hand. A close with 0 unmatched cards scores minus 10, or minus 50 when the hand is a 7-card run | Reward for one hand |
| 9 | Other players adding cards to melds on the table | On. It happens in the showdown described below | Value of closing early and of holding cards next to known melds |
| 10 | Counting the other players' unmatched cards | Each player chooses what to lay down in the showdown | Adds a short decision phase after every close |
| 11 | Conga (7-card run in one suit) | Scores minus 50 and does not end the match | A rare large bonus |
| 12 | Elimination | A player is out above 100 points. Exactly 100 stays in | Match objective |
| 13 | Re-entry after elimination | Off in version 1 | Endgame value of each point |
| 14 | Empty stock | Shuffle the discard pile except its top card into a new stock | Hand length and card counting |
| 15 | Discard pile | Only the top card can be taken. All past discards count as public knowledge | What each player knows |
| 16 | Dealer and tie-breaks | Random first dealer, then rotation. The player after the dealer starts. If all remaining players go out together, the lowest score wins | First-player advantage and edge cases |

### Showdown after a close

The showdown is written here as four steps taken from your description. Correct any step that differs from how you play.

1. The closer discards face down and play stops.
2. Starting with the closer and following turn order, each player may lay down any number of melds from their hand and add any number of single cards to melds on the table.
3. Turns continue around the table until a full round passes with no card placed.
4. Each player scores the value of the cards still in hand. Cards on the table cost nothing.

Simone confirmed these details on 16 November 2026.

| # | Detail | Rule |
| --- | --- | --- |
| a | Can the closer add cards to other players' melds in later rounds | Yes |
| b | Does a closer who empties their hand that way score minus 10 | No. Minus 10 needs 0 unmatched cards at the moment of closing |
| c | Must the closer lay down a split that meets the closing condition | Yes, and the server checks it |
| d | Can a player keep a meld in hand to give the others nothing to build on | Yes. The kept cards count as points |
| e | Does a player other than the closer earn minus 10 for an empty hand or minus 50 for a 7-card run | No. Both bonuses belong to the closer |
| f | Are hands hidden during the showdown | Yes. Only cards on the table are public until the count |

### What the confirmed rules change

- Scores can fall below zero, because of the minus 10 and minus 50 bonuses. Every table and model that takes a score must accept negative values.
- The other players can shed cards in the showdown, so an early close gains less than the raw card counts suggest.
- Taking a discard shows a card of yours, and opponents can keep cards that fit the meld you are building at no risk.
- The split a player lays down matters, because every meld on the table gives the next players places to add cards.
- The engine needs a third phase and more action indices. The architecture section has the new count.

### Rule clarifications

Edge-case readings of the rules above that the engine encodes. Each is custom to this variant and is listed so later readers can see what the engine actually does when the printed rule is ambiguous. Each clarification links to the decision in `docs/decisions.md`.

| # | Rule touched | Clarification | Reason |
| --- | --- | --- | --- |
| C1 | Rule 6 | `Close(c)` with the card just taken from the top of the discard pile is **legal**. Rule 6 forbids only the face-up re-discard (the "free pass"). A close is a face-down discard that ends the hand, so rule 6's purpose does not apply. See D15 | Keeps a technically-legal move available; removing it would only prune a rarely-useful corner case |
| C2 | Showdown detail (c) | The closer's `Pass` in their first showdown turn is legal **only** when their remaining hand is itself a closing residual (≤ 2 cards summing ≤ 5). They must physically lay down the melds of some closing split; a 7-card hand whose best split happens to close is not enough. See D16 | "Lay down a split" in detail (c) implies physical placement. The alternative (hand's best split is enough on its own) lets the closer hide their melds, which contradicts the server-check wording and hands the closer a strategic edge the design doc did not contemplate |
| C3 | Rules 4 and 16 | **The starter (seat after the dealer) is dealt one extra card.** The discard pile begins empty. The starter's first action is a `Discard`, not a `Draw` + `Discard`. Subsequent turns and all other seats follow the standard draw-then-discard structure in rule 4. The first-turn close ban in rule 5 still applies. See D17 | How Simone's table plays it. Equivalent to "deal 7 to each, turn the top card as the discard, starter draws it" in terms of hand sizes, but keeps the extra card private (no face-up card shown at the start of the hand) |

## Architecture

One authoritative game server sits between every client and a pure rules engine. Local, LAN, online and AI play then differ only in which clients connect and over which transport.

```
          +--------------------+
          | Clients (TUI, GUI) |
          +---------+----------+
                    |
              Protocol messages
                    |
          +---------v----------+
          |     Transport      |   in-memory, later WebSocket
          +---------+----------+
                    |
          +---------v----------+        +-----------+
          |    Game server     +------->| Event log |
          | validates, logs    |        +-----+-----+
          +---------+----------+              |
                    |                         | reads logs
              rule queries                    |
                    |                   +-----v-----+
          +---------v----------+        | AI library|
          |    Core engine     +<-------+ simulates |
          | state, rules       |        +-----------+
          +--------------------+
```

Read it top down: clients send actions and receive observations, the server validates and logs, the engine holds the rules. The AI library reads logged games and simulates with the same engine.

### Components

| Component | Responsibility | Must never |
| --- | --- | --- |
| Core engine | State, legal actions, transitions, scoring, observations, meld solver | Do input or output, read a clock or use an unseeded random source |
| Game server | Seats, turn loop, match scores, validation, event log | Contain rule logic or trust a client |
| Protocol | Serializable messages: action, observation, event, error | Send hidden information to the wrong seat |
| Transport | Moves messages. In-memory first, WebSocket later | Change a message |
| Clients (TUI, GUI) | Show observations and collect one action | Compute rules or hold the full state |
| AI library | Belief tracker, search, value model, explanations | Read the full game state during play |
| Event log | Ruleset id, shuffle results, ordered actions | Depend on engine internals that may change |

### Core interfaces

The signatures below are a sketch in Python notation for brevity, and the engine itself is Rust. Treat the names as placeholders and the boundaries as the design.

```python
# Engine: pure functions over a state that is cheap to copy
new_hand(rules, seed, scores, dealer) -> State
legal_actions(state) -> list[Action]
apply(state, action) -> tuple[State, list[Event]]
observe(state, seat) -> Observation   # the only place hidden information is filtered
is_terminal(state) -> bool
hand_points(state) -> list[int]

# AI support
sample_state(observation, rng) -> State   # a full state consistent with what the seat knows
encode(observation) -> Tensor             # seat-relative, one fixed size for 2 to 4 players

# One interface for human, AI and remote seats
class Player:
    async def act(self, observation, legal_actions) -> Action: ...
    def notify(self, event) -> None: ...

# Optional: what the explanation tools and a hint mode call
class Analyst:
    def analyse(self, observation) -> list[ActionReport]: ...   # value and reasons per legal action
```

The engine itself uses the Rust `RuleSet` trait defined in the Pluggable rule sets section below, which supersedes this Python sketch.

### Action space

A hand has three phases: draw, discard and showdown. The whole game fits in 333 fixed action indices, and the phase decides which ones are legal. The count is for your variant, and each rule set derives its own from its deck.

| Actions | Count | Phase |
| --- | --- | --- |
| Draw from the stock, take the top discard | 2 | Draw |
| Discard card c | 40 | Discard |
| Close with card c as the discard | 40 | Discard |
| Lay down meld m (50 possible sets, 120 possible runs of 3 to 7 cards) | 170 | Showdown |
| Add card c to a set or to a run on the table | 80 | Showdown |
| Pass | 1 | Showdown |

A fixed index space lets the logs, the search and a neural policy share one encoding.

### Design rules

1. Make the server authoritative from the first day. A local game runs the same server with an in-memory transport.
2. Derive every observation from the state and the seat with one function. That function stops the AI from peeking and stops the network from leaking.
3. Store a game as ruleset id, every shuffle result and action list. Replay, save and load, reconnect, bug reports and training data all come from that record.
4. Keep a seat separate from a player. A seat can be held by a human, an AI or nobody, and it can change hands during a match.
5. Make `act` asynchronous. A human takes seconds, a remote player can time out and an AI thinks for a fixed budget.
6. Model three nested levels: hand, match and session. The AI trains on hands and is judged on matches.
7. Stamp every message, log and trained model with a protocol version and a ruleset hash.
8. Encode a card as a small integer and a hand as a 64-bit mask, which covers any deck up to 64 cards. Compact state keeps simulation fast and copies cheap.

### Pluggable rule sets

A rule set is one Rust type that implements the `RuleSet` trait, and everything above the engine is written against that trait. Changing the rules then means adding a module to `conga-rules` and registering it.

```rust
pub trait RuleSet: Clone + Send + Sync + 'static {
    /// The full hidden state. Only the rule set reads or changes it.
    type State: Clone + Send;

    /// Name, version and parameter hash. Stamped on logs and trained models.
    fn id(&self) -> RuleSetId;
    fn deck(&self) -> DeckSpec;            // up to 64 cards, so a hand fits in a u64
    fn seats(&self) -> RangeInclusive<u8>; // allowed player counts

    fn new_hand(&self, scores: &[i32], dealer: Seat, chance: &mut dyn Chance) -> Self::State;
    fn legal_actions(&self, state: &Self::State, out: &mut ActionList);
    fn apply(&self, state: &mut Self::State, action: Action,
             chance: &mut dyn Chance, events: &mut Vec<Event>);
    fn observe(&self, state: &Self::State, seat: Seat) -> Observation;
    fn hand_result(&self, state: &Self::State) -> Option<HandResult>; // None while the hand runs
    fn match_status(&self, scores: &[i32]) -> MatchStatus;            // who is out, who has won

    /// Best split, unmatched total and whether the hand can close. Used by agents.
    fn evaluate_hand(&self, hand: Hand) -> HandEval;
    /// A full state consistent with what one seat knows. Used by the search.
    fn sample_state(&self, obs: &Observation, rng: &mut dyn Rng) -> Self::State;
}
```

`Chance` supplies every shuffle. It is random during play and reads the log during a replay.

#### Three sizes of rule change

| Change | Example | What you write |
| --- | --- | --- |
| A number or a switch | Score limit, closing sum, conga bonus, re-entry on or off | A new parameter value and no code |
| One part of the game | Another showdown, another closing test, jokers in melds | One piece that implements a small trait, plugged into the standard rule set |
| Another game flow | A variant with partners or with no discard pile | A full `RuleSet` implementation built from the toolkit |

The standard rule set, `StandardConga`, is assembled from five pieces: meld rules, closing rule, showdown rule, scoring and match rule. Your variant is `StandardConga` with your parameters and your showdown.

#### What stays fixed for every rule set

- Cards are small integers and hands are 64-bit masks.
- `Action`, `Observation` and `Event` are shared types. The server, the protocol, the TUI and the AI all read them.
- Play is a sequence of decisions, each taken by one seat.

A rule that needs a new kind of action or a new field in the observation is a change to `conga-core`, and it raises the protocol version.

#### What a new rule set gets for free

- Works unchanged: the server, the protocol, logs and replay, the TUI, belief sampling, the search and the evaluation harness.
- Must be redone: value model training, the match equity table, validation of every tactic and tuning of the rule-based agents.

Baseline agents call `evaluate_hand` to learn whether a hand can close and how it splits. No agent hard-codes your closing rule.

#### Dispatch

Hot code is generic, as in `fn rollout<R: RuleSet>(...)`, so each rule set compiles to its own fast path. The CLI, the server lobby and the Python bindings pick a rule set by name through one enum of registered rule sets. Registering a new one is one line in that enum.

#### Conformance tests

Every rule set must pass the same generic property tests before anything else uses it.

1. All cards of the deck are accounted for after every action.
2. No observation contains a card its seat should not see.
3. A hand that is still running always has a legal action.
4. A logged game replays to identical states.
5. Every sampled state reproduces the observation it was sampled from.

## Play modes

Every mode is the same server with a different set of seats and a different transport. No mode needs a change to the engine or to the AI.

| Mode | Where the server runs | Seats | Transport | Extra work |
| --- | --- | --- | --- | --- |
| Human against AI | Inside the TUI process | 1 human, 1 to 3 AI | In-memory | None after version 1 |
| Local multiplayer on one screen | Inside the client process | 2 to 4 humans, one client | In-memory | A hand-over screen that hides the previous hand |
| LAN | On one player's machine | Remote clients | WebSocket | Entering or discovering the host address, reconnect |
| Online | On a hosted machine | Remote clients | WebSocket over TLS | Lobby, identity, turn timers, hosting |
| Mixed table | Any of the above | Humans plus AI seats | Any | None |
| Spectating and replay | Any | A read-only seat | Any | A spectator observation, public or full with a delay |
| Self-play for training | No server | Agents only | Direct engine calls | None. The loop skips the server for speed |

Six protocol details cost little now and are painful to add later:

1. Number every event per seat. Reconnect then means "send me everything after event n".
2. Put the turn number in every action. The server can then reject a duplicate or late message.
3. Reserve fields for a turn timer and a timeout policy, even if version 1 ignores them.
4. Let an AI take over a seat when its player disconnects or times out.
5. Keep the seed and the stock order on the server. No client message ever contains them.
6. Make hints a server-side permission. A hint mode that is fine against the AI is cheating at a shared table.

## Software components

Rust runs the game and the search, Python trains models and analyses results, and one binding crate joins them. Library status was checked on 15 November 2026.

### Workspace

One Cargo workspace holds the Rust crates, and one Python package sits beside it.

| Unit | Language | Role | Main libraries |
| --- | --- | --- | --- |
| `conga-core` | Rust | Shared types, the `RuleSet` trait, the toolkit for decks, melds and scoring, and the conformance tests. Synchronous and free of input or output | `rand_core`, optional `serde` |
| `conga-rules` | Rust | One module per rule set, with your variant as the first | `conga-core` only |
| `conga-ai` | Rust | Baseline agents, belief tracker, search, rollouts. Generic over the rule set | `rayon` |
| `conga-protocol` | Rust | Message types and versions | `serde`, `serde_json` |
| `conga-server` | Rust | Seats, turn loop, match scores, event log, transports | `tokio`, later `axum` with its `ws` feature |
| `conga-tui` | Rust | Terminal client | `ratatui`, `crossterm` |
| `conga-cli` | Rust | Tournaments, evaluation runs, log export | `clap`, `rayon` |
| `conga-py` | Rust | Python bindings for the core and the AI | `pyo3`, `maturin` |
| `conga-lab` | Python | Training, explanation tools, notebooks | PyTorch, scikit-learn, polars, matplotlib |
| Web client | Later | Browser GUI | `conga-core` compiled to WebAssembly with `wasm-bindgen` |

Tests use `proptest` for the engine invariants, `insta` for golden games and `criterion` for the rollout benchmark.

Two dependency rules hold the design together. `conga-core` depends on no other crate in the workspace. `conga-ai` receives observations and builds its own sampled states, and it never receives the real state held by the server.

### Findings from the library check

| Topic | Finding | Consequence |
| --- | --- | --- |
| Terminal UI | [Ratatui 0.30](https://ratatui.rs/highlights/v030/) is the current release line and split the library into modular crates | Use `ratatui` with the `crossterm` backend |
| Python bindings | [PyO3 0.29](https://github.com/pyo3/pyo3/releases) came out in June 2026 with Python 3.15 support, and maturin builds the wheels | A mature path. Use `maturin develop` during daily work |
| Model inference in Rust | [`ort`](https://docs.rs/crate/ort/latest/source) is still a release candidate (2.0.0-rc.13, July 2026). It also offers pure Rust backends | Pin the exact version if you use it. Nothing needs it before step 7 |
| OpenSpiel | [Version 2.0](https://github.com/google-deepmind/open_spiel/releases/tag/v2.0) marks its Rust API as unmaintained | Avoid building on that API. Reach OpenSpiel algorithms through a Python game wrapper if they are ever needed |
| Learning algorithm | A [2026 benchmark](https://arxiv.org/abs/2502.08938v3) found tuned PPO-type methods at least as strong as NFSP, PSRO, ESCHER and R-NaD on five test games. Its authors warn that five games are a narrow base | Use PPO self-play with strong entropy regularisation as the learning method. Leave NFSP and Deep CFR out of the plan |
| Random numbers | The `rand` book advises against `StdRng` and `SmallRng` when results must be reproducible, and minor releases may change output values | Use a named generator such as ChaCha8, write the shuffle by hand and log every shuffle result |
| Multi-agent API | [PettingZoo 1.26](https://pettingzoo.farama.org/release_notes) is maintained and supports Python 3.14 | Wrap the bindings in its turn-based API only if a third-party training library is used |
| WebSocket server | `axum` 0.8 has built-in WebSocket support on top of `tokio` | Use it when LAN play starts |

### The boundary between Rust and Python

1. Training never calls the engine one action at a time. Rust plays many games in parallel and returns arrays of encoded observations, legal-action masks and results.
2. Until step 7 the network runs only in Python. Rust asks for actions in batches, as a vector environment does.
3. From step 7 the search needs the value model inside Rust. Choice 2 below covers the options.
4. Explanation tools call the Rust search through the bindings and read the event logs. They hold no rule logic of their own.

### Choices still open for discussion

| # | Choice | Options | Current lean |
| --- | --- | --- | --- |
| 1 | Where the search lives | Rust in `conga-ai`. Python on top of the bindings | Decided on 15 November 2026: Rust, because search quality grows with rollouts per second |
| 2 | Value model inside the Rust search | A hand-written forward pass for a small network. ONNX through `ort` or `tract`. Training in Rust with `burn` | Hand-written forward pass first. It takes a few dozen lines and also runs in WebAssembly. Move to ONNX if the model grows |
| 3 | Wire and log format | JSON. A binary format such as MessagePack or `postcard` | JSON. Logs stay readable and the messages are small |
| 4 | Training loop | An own PPO loop in the single-file style of CleanRL. A training library behind a PettingZoo wrapper | Own loop. Self-play against a pool of past versions is easier to control |
| 5 | Web client | A TypeScript interface with the core in WebAssembly. An all-Rust interface | Decide at step 9. The core compiles to WebAssembly in both cases |
| 6 | Showdown rules in Rust | Write the phase in Rust at once. Prototype it in a notebook first | Rust at once, after the six showdown details are settled |

## AI plan

The main agent searches over sampled opponent hands, guided by a belief tracker and later by a learned value model. It gives a value for every legal action, and the explanation work is built on those values.

### What makes Conga hard for an AI

| Property | Consequence |
| --- | --- |
| Hidden hands and hidden stock | The agent reasons over beliefs. There is no single position to search |
| High luck | Comparing two agents needs thousands of matches |
| Match scoring with elimination | The best action depends on the scores, above all the choice to close |
| 2 to 4 players | Equilibrium theory holds for 2. With 3 or 4, one player's choice can decide between two others |
| Only the next player can take a discard | Discard safety is about one opponent at a time |
| 333 fixed actions, about 8 legal per decision while playing | Search per decision is cheap |
| A showdown after every close | Every hand ends with a short decision game, and the points depend on what the others lay down |

### Building blocks, in build order

1. **Meld solver.** For 7 or 8 cards it returns the best split, the unmatched total, whether the hand can close and the unseen cards that would improve it. Exact enumeration is enough at this size.
2. **Baseline agents.** Random, greedy (lowest unmatched total, close at once) and rule-based (greedy plus discard safety). They form the ladder every later agent must climb.
3. **Belief tracker.** It records cards an opponent took from the discard pile and still holds, cards they discarded and top discards they declined. Version 1 samples uniformly among deals that fit these facts. Version 2 weights samples by how likely the opponent's actions were under a model of their play.
4. **Search agent.** It samples full states that fit the observation, plays each legal action forward with a rollout policy and averages the match value. Rollout players act on their own observations only. A rollout that lets players see all cards would undervalue hiding a hand.
5. **Match value.** Each rollout ends with points per player. A match equity table turns the new scores into a win probability, as in backgammon. Scores can fall below zero, so the 2-player table needs a negative range with a floor. For 3 or 4 players it needs a fitted function.
6. **Value model.** A network trained on self-play predicts match value from an observation and replaces most rollouts. Its input is seat-relative and has one fixed size for 2 to 4 players.

### Showdown play

The showdown is a short game of its own with hidden cards, and the engine exposes it through the same legal actions as the rest of the hand. Version 1 of the agent plays it by exhaustive search over sampled hidden cards. Rollouts use a fast greedy showdown policy, and its bias on hand values must be measured early.

### Other approaches and their role

| Approach | Role here | Weakness |
| --- | --- | --- |
| Self-play reinforcement learning (PPO or DQN) | A second opinion and a stronger rollout policy | Needs tuning and strong entropy regularisation to avoid cycling. Gives no per-action reasons |
| NFSP or Deep CFR | Optional. A 2026 benchmark found tuned PPO-type methods at least as strong | Long hands and a huge information-set space make training heavy |
| ISMCTS | A replacement for the flat search if it misjudges information plays | More tuning for an uncertain gain |

### Evaluation

- Play every deal twice with seats swapped. Duplicate deals remove much of the luck from a comparison.
- Report match win rate with a confidence interval. 10,000 independent matches give about plus or minus 1 percentage point at 95%.
- Require each new agent to beat the previous rung with a clear margin before moving on.
- For 3 and 4 players, seat one copy of agent A among copies of agent B, then the reverse, over all seat orders.
- Estimate exploitability by training a fresh agent against the frozen one. Its win rate is a lower bound on the weakness.
- Play against it yourself and log the games. Human games are the only check on whether its opponents are realistic.

### Simulation speed

A search decision costs samples times legal actions times one rollout. With 100 samples and 8 legal discards that is 800 rollouts per decision. Measure rollouts per second in the first week, because that number sets how many samples the search can afford.

## Explainability and tactic discovery

A tactic counts as found only when adding it as a rule to a baseline agent raises that agent's win rate. Every method below produces candidates for that test.

### Methods

| Method | What it gives | Limit |
| --- | --- | --- |
| Per-action values from search | A win probability for each legal action in a position | Shows which action is better and gives no reason |
| Value decomposition | The gap between two actions split into parts: own hand progress, risk of feeding the next player, information revealed | The parts are defined by hand |
| Counterfactual probes | Change one fact (a score, a known card, the turn number) and see if the choice flips | Tests one fact at a time |
| Disagreement mining | Positions where the strong agent and the greedy baseline differ, grouped by features | Finds only what the features can express |
| Distillation | A small decision tree or rule list that imitates the agent, with its fidelity measured | A tree with low fidelity explains little |
| Targeted experiments | One question answered by simulation, such as the best closing threshold per score | Slow, and the most reliable |
| Feature attribution on the value model | A quick view of what the network uses | Can be unfaithful. Treat it as a hint |

### Feature vocabulary

Explanations are written in features a player already thinks in. The list below is the starting set.

| Group | Features |
| --- | --- |
| Own hand | Unmatched total, number of unmatched cards, melds held, cards one draw away from a meld, unseen cards that would help |
| Risk | Chance that the next player can use each possible discard |
| Information | Which of my cards the opponents know, which of theirs I know |
| Context | Turn number, stock size, scores, each player's distance to elimination |
| Showdown | Cards in hand that fit melds an opponent is known to hold, openings my own melds give to the players after me |

### First questions for the system

1. When should a player close at the first chance, and when should they play on for a close with 0 unmatched cards?
2. How does that answer move with the match scores?
3. When is taking the top discard worth showing a card to the table?
4. When should high cards go early, and when are they worth keeping for a meld?
5. When is it right to hold a useless card to deny the next player?
6. How much is the first seat worth?
7. When is chasing the 7-card run worth the risk?

The showdown adds two questions. Which split of a hand leaves the fewest openings for the players who follow? When is it worth keeping a card that fits a meld the likely closer is building?

### Validation loop

1. Write the candidate tactic as a rule over the features.
2. Add the rule to the rule-based agent.
3. Play duplicate matches against the unchanged agent and against the strong agent.
4. Keep the tactic if the gain clears the confidence interval.
5. Record where it holds: ruleset, player count and score range.

### Limits to state with every result

- The agent remembers every discard. Tag each tactic with the memory it needs, because a person may not have it.
- Tactics are best responses to the opponents used in training. Without human game logs their value against people stays untested.
- Optimal play with hidden information can be randomised. A tactic phrased as "always do X" may approximate a mix.

## Decisions to make now

Eleven decisions shape code that everything else imports, so changing them later means rework. Each has a recommendation you can accept or overrule.

| # | Decision | Options | Recommendation | Why it cannot wait |
| --- | --- | --- | --- | --- |
| 1 | Showdown details | The 6 showdown details in the ruleset section | Confirm or change each proposal | The engine and every trained model depend on them |
| 2 | Language | Python only. A Rust core with Python and WebAssembly bindings. Python first with a later port of the engine | Rust for the engine, server, TUI and search. Python for training and analysis through bindings | Every component imports the engine |
| 3 | Player count | 2 only. 2 to 4 everywhere | Engine, protocol and encoding support 2 to 4. The AI is trained and validated on 2 first | The encoding shape is hard to change after training |
| 4 | Objective | Points per hand. Match win probability | Match win probability through an equity table | It sets the reward, the value target and the metric |
| 5 | Opponents that define "best play" | Pure self-play. A fixed pool. Human-like models | A pool of the baselines plus earlier versions of the agent. Add human logs when they exist | It decides what a tactic claims to beat |
| 6 | Information model | Full discard history is public. Only the top card is known | Full history, with a memory tag on each tactic | It fixes the `Observation` type |
| 7 | State representation | Objects and lists. Integers and bitmasks | Integers and bitmasks | Speed, cheap copies and an easy port |
| 8 | Randomness | Shuffle the whole deck from a seed at the deal. Sample each draw when it happens | Shuffle from a seed at the deal | Exact replay depends on it |
| 9 | Log format | Any | JSON lines with protocol version, ruleset hash, every shuffle result and actions | Logs from the first day become training data |
| 10 | Compute budget | Laptop CPU. One GPU. Rented machines | State the budget you have | It bounds samples per decision and network size |
| 11 | Definition of done | Any | Set a win-rate target against the rule-based agent and a number of validated tactics | Without it the project has no end |

Decision 2 now follows your preference for Rust. The software components section lists the crates and libraries, and its last table holds the choices still open.

## Decisions that can wait

Nine decisions can wait, because an interface defined now already isolates each of them.

| Decision | Why it can wait | What keeps it cheap later |
| --- | --- | --- |
| GUI or web framework | A client only shows observations and returns one action | The protocol |
| Network library and hosting | The in-memory transport implements the same interface | The transport boundary |
| Accounts, lobby, matchmaking | The server knows seats and has no notion of identity | Seats kept separate from players |
| Value model architecture | The search agent works with rollouts first | The `Analyst` interface and `encode` |
| AI for 3 and 4 players | The encoding is already sized for 4 seats | Seat-relative encoding |
| Rule variants such as re-entry | Each one is a parameter or a piece of a `RuleSet` | The ruleset hash on logs and models |
| WebAssembly build of the engine | Only a browser client needs it | A core crate with no system dependencies |
| Shuffle fairness proofs and anti-cheat | They matter only for public online play | The authoritative server |
| Collecting human games and consent | It needs online play first | The log format |

## Gaps and risks

Thirteen points are not addressed yet. The first four can invalidate results, so they come before any tactic claim.

| # | Gap or risk | Why it matters | Proposed handling |
| --- | --- | --- | --- |
| 1 | Engine bugs | A wrong rule silently corrupts every model and tactic | Property tests: all 40 cards accounted for after every action, no hidden card in any observation, exact replay from a log. Play test hands by hand |
| 2 | False tactics from luck | Testing many candidates produces some winners by chance | Confirm every kept tactic on a fresh set of seeds |
| 3 | No reference for strength | This variant has no known strong agent to compare with | The baseline ladder, the exploitability estimate and your own games |
| 4 | Hidden information leaking into the AI | An agent that sees the full state looks brilliant and teaches nothing | The AI library accepts observations only. Enforce it with types and an import rule |
| 5 | No model of human opponents | Tactics are proven only against your own agents | Log every human game from the first TUI match |
| 6 | Agent tuned to its own style | Rollouts assume opponents play like the rollout policy | Evaluate against a varied pool of opponents |
| 7 | Feature bias | Only tactics the features can express will be found | Inspect high-value disagreements the features fail to separate, then add features |
| 8 | Cost of crossing between Rust and Python | One call per action across the boundary makes training crawl | Rust plays whole batches of games and hands arrays to Python |
| 9 | Theory for 3 and 4 players | No equilibrium guarantee and results depend on who sits where | Report results per seating and avoid claims of optimal play |
| 10 | Match equity for 3 and 4 players | A lookup table is too large | A fitted function, trained from simulated matches |
| 11 | Audience for the tactics | The form of an explanation depends on who reads it | Decide: notes for yourself, a hint mode in the game or a written guide |
| 12 | Scope drift toward frontends | Network and GUI work can absorb the time meant for the AI | No network code before the search agent beats the rule-based agent |
| 13 | Informal showdown rules | You play this stage by feel and the engine needs exact rules. Each way of writing them down produces different tactics | Settle the six details, then replay hands you remember and check that the engine agrees |

## Milestones

The work runs in ten steps, and each step ends with a test that can fail. Network play comes last on purpose.

Status (2026-11-18): steps 0, 1 and 2 are accepted (tags `setup-accepted`, `m1-accepted`, `m2-accepted`). Step 3 is in progress on branch `m3-baselines-eval` with the plan at `docs/plans/m3.md`.

| Step | Deliverable | Exit test |
| --- | --- | --- |
| 0 | Confirmed ruleset | All 6 showdown details answered and stored in a `RuleSet` |
| 1 | Core engine with the rule set trait, the meld solver and your variant as the first rule set | The conformance tests pass on a large batch of random games. A logged game replays to identical states |
| 2 | Game server, protocol, in-memory transport, TUI | You play a full match against the random agent and replay it from its log |
| 3 | Baseline agents and evaluation harness | Random loses to greedy and greedy loses to rule-based, each outside the confidence interval. Rollouts per second measured (recorded as "decisions per second" for baseline agents, see D30) |
| 4 | Belief tracker and search agent | The search agent beats the rule-based agent outside the confidence interval |
| 5 | Match equity and score-aware play | The score-aware agent beats its score-blind version over full matches |
| 6 | Explanation tools and the validation loop | The first tactic passes the loop on fresh seeds |
| 7 | Value model | Equal strength at lower cost per decision, or higher strength at equal cost |
| 8 | 3 and 4 players, exploitability estimate | Results reported per seating for each player count |
| 9 | Network transport, LAN then online | A LAN match and a reconnect in the middle of a hand both work with no engine change |
