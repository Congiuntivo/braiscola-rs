#!/usr/bin/env bash
set -euo pipefail

json_file="${1:-examples/advisor/respond_turn_n.json}"
samples="${2:-128}"
seed="${3:-42}"

cargo run -p cli --bin advisor -- suggest --json "${json_file}" --samples "${samples}" --seed "${seed}"
