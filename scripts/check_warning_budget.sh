#!/usr/bin/env bash
# Verdict: WIRE
# Gate: `scripts/preflight.sh` runs this as `warning budget`.
# Route Cargo through the repository wrapper so the gate works both inside and
# outside the Nix development shell. Suppress Nix's dirty-tree notice because it
# starts with `warning:` but is not a Rust compiler warning.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
baseline_file="$repo_root/scripts/warning_budget.txt"

usage() {
  cat <<'USAGE'
Usage:
  scripts/check_warning_budget.sh            # fail if warnings exceed baseline
  scripts/check_warning_budget.sh --update   # update baseline to current warning count

Notes:
  - Counts Rust compiler lines that begin with "warning:" from `cargo check -q`
  - Baseline is stored in scripts/warning_budget.txt
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ ! -f "$baseline_file" ]]; then
  echo "error: missing baseline file: $baseline_file" >&2
  exit 1
fi

# Count with grep, not rg. This step ran for the entire life of the gate as
# `... | rg -c '^warning:' || printf '0\n'` on a runner that has no ripgrep, so
# `rg: command not found` was absorbed by the `||` and the gate printed
# "Warning budget OK: current=0 baseline=0" while counting nothing. Observed in
# CI, not inferred: run 30769227268, Quality Guardrails, lines 755-756.
#
# grep is in coreutils-adjacent base on every runner and in the devshell, so the
# tool cannot go missing the way rg did. `grep -c` still exits 1 on zero
# matches, which is the legitimate zero-warning case, so `|| true` is needed and
# is safe only because the cargo status is checked separately below.
#
# The cargo exit status is captured explicitly instead of being lost to a pipe:
# a compile failure emits no `warning:` lines, so the old shape reported a
# broken build as a clean budget too. Verified separately with a stub that
# exits 101; it printed current=0 and exited 0.
cd "$repo_root"
set +e
nix_config="${NIX_CONFIG:-}"
[[ -z "$nix_config" ]] || nix_config+=$'\n'
nix_config+='warn-dirty = false'
cargo_output=$(
  NIX_CONFIG="$nix_config" JCODE_REMOTE_CARGO=0 CARGO_TERM_COLOR=never \
    "$repo_root/scripts/dev_cargo.sh" check -q 2>&1
)
cargo_status=$?
set -e
if (( cargo_status != 0 )); then
  echo "error: cargo check failed (exit $cargo_status); warning count is not measurable" >&2
  printf '%s\n' "$cargo_output" >&2
  exit 1
fi
current=$(printf '%s\n' "$cargo_output" | grep -c '^warning:' || true)
baseline=$(tr -d '[:space:]' < "$baseline_file")

if [[ "${1:-}" == "--update" ]]; then
  printf '%s\n' "$current" > "$baseline_file"
  echo "Updated warning baseline: $baseline"
  echo "New warning baseline: $current"
  exit 0
fi

if ! [[ "$baseline" =~ ^[0-9]+$ ]]; then
  echo "error: invalid warning baseline in $baseline_file: '$baseline'" >&2
  exit 1
fi

if (( current > baseline )); then
  echo "Warning budget exceeded: current=$current baseline=$baseline" >&2
  echo "Run scripts/check_warning_budget.sh --update only after intentional cleanup." >&2
  exit 1
fi

if (( current < baseline )); then
  echo "Warning budget improved: current=$current baseline=$baseline"
  echo "Consider running: scripts/check_warning_budget.sh --update"
else
  echo "Warning budget OK: current=$current baseline=$baseline"
fi
