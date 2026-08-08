#!/usr/bin/env bash
# ci_local.sh - run the canonical CI-emulation recipe from the repo justfile.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root" || {
  printf 'ci_local: cannot cd to repo root %s\n' "$repo_root" >&2
  exit 2
}

job="macos"
target=""
list_only=0

while [ $# -gt 0 ]; do
  case "$1" in
    --job) job="${2:?--job needs a value}"; shift 2 ;;
    --target) target="${2:?--target needs a value}"; shift 2 ;;
    --list) list_only=1; shift ;;
    -h|--help)
      cat <<'EOF'
Usage: scripts/ci_local.sh [--job macos|linux-tests] [--target TRIPLE] [--list]

Runs the justfile recipe that mirrors the CI cargo command list.
EOF
      exit 0
      ;;
    *)
      printf 'ci_local: unknown argument %s\n' "$1" >&2
      exit 2
      ;;
  esac
done

if [ -z "$target" ]; then
  host_triple=$( (rustc -vV 2>/dev/null || true) | sed -n 's/^host: //p')
  target="${host_triple:-aarch64-apple-darwin}"
fi

recipe="full-test"
script=$(python3 "$repo_root/scripts/ci_workflow_commands.py" "$recipe")
if [ -z "$script" ]; then
  printf 'ci_local: no script extracted for recipe %s\n' "$recipe" >&2
  exit 1
fi

printf 'ci_local: job=%s recipe=%s target=%s (%d lines)\n' \
  "$job" "$recipe" "$target" "$(printf '%s\n' "$script" | wc -l | tr -d '[:space:]')"
printf '%s\n' "$script" | sed 's/^/  | /'

if [ "$list_only" -eq 1 ]; then
  exit 0
fi

printf '\n=== ci_local: running recipe %s\n' "$recipe"
JCODE_CI_TARGET="$target" bash -lc $'set -euo pipefail\n'"$script"
