#!/usr/bin/env bash
set -euo pipefail

seed="${1:-42}"

cargo run -p cli --bin simulate_game -- "${seed}"
