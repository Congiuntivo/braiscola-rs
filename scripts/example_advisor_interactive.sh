#!/usr/bin/env bash
set -euo pipefail

samples="${1:-128}"
seed="${2:-42}"

cargo run -p cli --bin advisor -- interactive --samples "${samples}" --seed "${seed}"
