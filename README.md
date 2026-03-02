# brAIscola-rs

A Rust implementation of a 2-player Briscola engine with hidden-information Monte Carlo move selection.

## Workspace Layout

- `briscola_core`: card model, scoring/power ordering, trick resolution, deterministic state transitions.
- `briscola_ai`: determinization from public information, heuristic rollout policy, root Monte Carlo evaluator.
- `cli`: binaries for move evaluation and full turn-by-turn game simulation.

## Implemented Rules

- 40-card Italian deck (`Coins`, `Cups`, `Swords`, `Clubs`) with ranks `A,2,3,4,5,6,7,J,Q,K`.
- Briscola scoring values: `A=11`, `3=10`, `K=4`, `Q=3`, `J=2`, others `0`.
- Trick strength ordering: `A > 3 > K > Q > J > 7 > 6 > 5 > 4 > 2`.
- Winner draws first; loser draws second.
- When one talon card remains, loser receives the face-up trump card as final draw.
- No obligation to follow suit; legal move set is always all cards in hand.

## AI Approach

At each decision:

1. Build unknown pool from `FULL_DECK - seen_cards`.
2. Sample hidden world consistent with:
   - opponent hand size,
   - talon length,
   - known face-up trump.
3. Force each candidate move at root.
4. Roll out to terminal state with Briscola heuristics for both players.
5. Aggregate `p_win` and expected score delta; choose argmax.

## Run

```bash
cargo test
cargo run -p cli
cargo run -p cli --bin simulate_game -- 42
cargo run -p cli --bin simulate_game -- 42 --best-me --samples 256
```

- `cargo run -p cli` prints the selected best move and statistics for every legal move.
- `cargo run -p cli --bin simulate_game -- <seed>` simulates an entire game and prints every trick.
- Add `--best-me` to have `Me` choose root Monte Carlo best moves each turn.
- Tune `--samples <N>` to trade speed for stronger move selection in `--best-me` mode.

## Quality Gates

The workspace enforces strict clippy/rust lints via `Cargo.toml` workspace lint settings and uses Rust 2024 style formatting.

```bash
cargo fmt-all
cargo fmt-check
cargo lint-strict
cargo verify
```
