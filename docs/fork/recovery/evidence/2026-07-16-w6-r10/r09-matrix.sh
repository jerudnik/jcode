#!/usr/bin/env bash
set -uo pipefail
out_dir="$1"
mkdir -p "$out_dir/r09"
export CARGO_NET_OFFLINE=true
export FORK_NUDGE_MAX_AGE=2147483647
export FORK_NUDGE_AUTOSYNC=0
export JCODE_NO_TELEMETRY=1
export JCODE_HOME
export JCODE_RUNTIME_DIR
JCODE_HOME="$(mktemp -d)"
JCODE_RUNTIME_DIR="$(mktemp -d)"
trap 'rm -rf "$JCODE_HOME" "$JCODE_RUNTIME_DIR"' EXIT
run_case() {
  local name="$1" expected="$2"; shift 2
  local log="$out_dir/r09/${name}.log"
  {
    printf 'name=%s\nexpected_exit=%s\ncommand=' "$name" "$expected"
    printf '%q ' "$@"
    printf '\n--- output ---\n'
  } > "$log"
  "$@" >> "$log" 2>&1
  local actual=$?
  printf '\nactual_exit=%s\n' "$actual" >> "$log"
  printf '%s expected=%s actual=%s\n' "$name" "$expected" "$actual" | tee -a "$out_dir/r09-summary.txt"
  if [ "$actual" -ne "$expected" ]; then
    return 1
  fi
  return 0
}
: > "$out_dir/r09-summary.txt"
status=0
run_case classifier 0 python3 -m unittest discover -s tests -p test_rust_production_filter.py || status=1
run_case panic 1 python3 scripts/check_panic_budget.py || status=1
run_case swallowed 1 python3 scripts/check_swallowed_error_budget.py || status=1
run_case prod_size 1 python3 scripts/check_code_size_budget.py || status=1
run_case test_size 1 python3 scripts/check_test_size_budget.py || status=1
run_case wildcard 0 python3 scripts/check_wildcard_reexport_budget.py || status=1
run_case warning 0 bash scripts/check_warning_budget.sh || status=1
exit "$status"
