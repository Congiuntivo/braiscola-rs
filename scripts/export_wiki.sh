#!/usr/bin/env bash
set -euo pipefail

out_dir="${1:-}"
if [[ -z "${out_dir}" ]]; then
  echo "usage: bash scripts/export_wiki.sh <output-dir>" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "${out_dir}"

cp "${repo_root}/README.md" "${out_dir}/Home.md"
cp "${repo_root}/plan/BRISCOLA.md" "${out_dir}/Game-as-Algorithm.md"
cp "${repo_root}/examples/advisor/README.md" "${out_dir}/Advisor-Examples.md"
cp "${repo_root}/scripts/README.md" "${out_dir}/Scripts.md"
cp "${repo_root}/res/napoletane/README.md" "${out_dir}/Card-Assets.md"

cat > "${out_dir}/_Sidebar.md" <<'EOF'
- [Home](Home)
- [Game as Algorithm](Game-as-Algorithm)
- [Advisor Examples](Advisor-Examples)
- [Scripts](Scripts)
- [Card Assets](Card-Assets)
EOF
