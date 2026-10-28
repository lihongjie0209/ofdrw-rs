#!/usr/bin/env bash
set -euo pipefail

ofdrw_rust_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ofdrw_rust_root"

case "${1:-all}" in
  all) ofdrw_coverage_crates=(ofdrw ofdrw-ffi) ;;
  ofdrw|ofdrw-ffi) ofdrw_coverage_crates=("$1") ;;
  *) printf 'Usage: %s [all|ofdrw|ofdrw-ffi]\n' "$0" >&2; exit 2 ;;
esac

for ofdrw_coverage_crate in "${ofdrw_coverage_crates[@]}"; do
  case "$ofdrw_coverage_crate" in
    ofdrw) ofdrw_coverage_threshold=90 ;;
    ofdrw-ffi) ofdrw_coverage_threshold=93 ;;
  esac
  # Old coverage objects can add unexecuted crates or duplicate source mappings.
  cargo llvm-cov clean --workspace
  cargo llvm-cov -p "$ofdrw_coverage_crate" --locked \
    --fail-under-lines "$ofdrw_coverage_threshold" --summary-only
done
