# Scripts Index

This directory contains thin wrappers around common `cargo run` commands.

## Script Mapping

- `example_move_eval.sh`
  - Runs: `cargo run -p cli --bin cli`
  - Purpose: evaluate one built-in decision state.

- `example_simulate.sh [seed]`
  - Default seed: `42`
  - Runs: `cargo run -p cli --bin simulate_game -- <seed>`
  - Purpose: full-game simulation with heuristic policy.

- `example_simulate_best_me.sh [seed] [samples]`
  - Defaults: `seed=42`, `samples=256`
  - Runs: `cargo run -p cli --bin simulate_game -- <seed> --best-me --samples <samples>`
  - Purpose: full-game simulation with Monte Carlo policy for Me.

- `example_advisor_suggest.sh [json] [samples] [seed]`
  - Defaults: `examples/advisor/respond_turn_n.json`, `128`, `42`
  - Runs: `cargo run -p cli --bin advisor -- suggest --json <json> --samples <samples> --seed <seed>`
  - Purpose: stateless next-move suggestion from JSON snapshot.

- `example_advisor_interactive.sh [samples] [seed]`
  - Defaults: `128`, `42`
  - Runs: `cargo run -p cli --bin advisor -- interactive --samples <samples> --seed <seed>`
  - Purpose: interactive turn-by-turn advisor session.

- `example_play_tui.sh [seed] [hint_samples] [opponent_samples]`
  - Defaults: `42`, `128`, `96`
  - Runs: `cargo run -p cli --bin play_tui -- --seed <seed> --hint-samples <hint_samples> --opponent-samples <opponent_samples>`
  - Purpose: full interactive TUI game against AI.

- `run_all_examples.sh`
  - Runs all non-interactive examples in sequence and prints section headers.
  - The TUI command is printed as a manual follow-up step.

## Usage

Run scripts from repository root:

```bash
bash scripts/example_move_eval.sh
bash scripts/example_simulate.sh 99
bash scripts/example_simulate_best_me.sh 99 256
bash scripts/example_advisor_suggest.sh examples/advisor/lead_turn_n.json 192 42
bash scripts/run_all_examples.sh
```
