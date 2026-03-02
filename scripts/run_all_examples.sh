#!/usr/bin/env bash
set -euo pipefail

echo "== move evaluation =="
bash scripts/example_move_eval.sh

echo "== random game simulation =="
bash scripts/example_simulate.sh 42

echo "== simulation with best-me mode =="
bash scripts/example_simulate_best_me.sh 42 128

echo "== advisor suggest: respond_turn_n =="
bash scripts/example_advisor_suggest.sh examples/advisor/respond_turn_n.json 128 13

echo "== advisor suggest: lead_turn_n =="
bash scripts/example_advisor_suggest.sh examples/advisor/lead_turn_n.json 192 42

echo "== advisor suggest: endgame_turn_n =="
bash scripts/example_advisor_suggest.sh examples/advisor/endgame_turn_n.json 256 7

echo "== interactive TUI example =="
echo "run manually: bash scripts/example_play_tui.sh 42 128 96"
