#!/usr/bin/env bash
set -euo pipefail

seed="${1:-42}"
hint_samples="${2:-128}"
opponent_samples="${3:-96}"

cargo run -p cli --bin play_tui -- --seed "${seed}" --hint-samples "${hint_samples}" --opponent-samples "${opponent_samples}"
