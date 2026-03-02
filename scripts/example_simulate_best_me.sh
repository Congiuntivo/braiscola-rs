#!/usr/bin/env bash
set -euo pipefail

seed="${1:-42}"
samples="${2:-256}"

cargo run -p cli --bin simulate_game -- "${seed}" --best-me --samples "${samples}"
