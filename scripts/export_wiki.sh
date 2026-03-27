#!/usr/bin/env bash
set -euo pipefail

out_dir="${1:-}"
if [[ -z "${out_dir}" ]]; then
  echo "usage: bash scripts/export_wiki.sh <output-dir> [api-docs-dir]" >&2
  exit 2
fi

api_docs_dir="${2:-}"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "${out_dir}"

cp "${repo_root}/README.md" "${out_dir}/Home.md"
cp "${repo_root}/plan/BRISCOLA.md" "${out_dir}/Game-as-Algorithm.md"
cp "${repo_root}/examples/advisor/README.md" "${out_dir}/Advisor-Examples.md"
cp "${repo_root}/scripts/README.md" "${out_dir}/Scripts.md"
cp "${repo_root}/res/napoletane/README.md" "${out_dir}/Card-Assets.md"

if [[ -n "${api_docs_dir}" && -d "${api_docs_dir}" ]]; then
  mkdir -p "${out_dir}/Rust-API"
  cp -R "${api_docs_dir}/." "${out_dir}/Rust-API/"

  cat > "${out_dir}/Rust-API.md" <<'EOF'
# Rust API Docs

This page links to API markdown generated from rustdoc comments.

- [API Index](Rust-API/index)
- [briscola_core](Rust-API/briscola_core/index)
- [briscola_ai](Rust-API/briscola_ai/index)
- [cli](Rust-API/cli/index)

These files are generated in CI and synced to the wiki.
EOF
fi

cat > "${out_dir}/_Sidebar.md" <<'EOF'
- [Home](Home)
- [Game as Algorithm](Game-as-Algorithm)
- [Advisor Examples](Advisor-Examples)
- [Scripts](Scripts)
- [Card Assets](Card-Assets)
- [Rust API](Rust-API)
EOF
