#!/usr/bin/env bash
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
out="$repo/docs/fork/recovery/evidence/2026-07-16-w5-onboarding-consent"
mkdir -p "$out"
cd "$repo"

rust_bin=/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin
python_bin=/Library/Developer/CommandLineTools/usr/bin/python3
if [[ ! -x "$rust_bin/cargo" ]]; then
  echo "required cached cargo missing: $rust_bin/cargo" >&2
  exit 1
fi
if [[ ! -x "$rust_bin/rustfmt" ]]; then
  echo "required cached rustfmt missing: $rust_bin/rustfmt" >&2
  exit 1
fi
if [[ ! -x "$python_bin" ]]; then
  echo "required system python missing: $python_bin" >&2
  exit 1
fi
export PATH="$rust_bin:$PATH"
export CARGO_NET_OFFLINE=true
export CARGO_INCREMENTAL=0
export JCODE_NO_TELEMETRY=1
export FORK_NUDGE_MAX_AGE=2147483647
export FORK_NUDGE_AUTOSYNC=0
export CARGO_TARGET_DIR=/tmp/jcode-w5-consent-target
export JCODE_HOME
export JCODE_RUNTIME_DIR
JCODE_HOME=$(mktemp -d /tmp/jcode-w5-home.XXXXXX)
JCODE_RUNTIME_DIR=$(mktemp -d /tmp/jcode-w5-runtime.XXXXXX)
cleanup() {
  rm -rf "$JCODE_HOME" "$JCODE_RUNTIME_DIR"
}
trap cleanup EXIT

echo "repo=$repo" >"$out/run.meta"
echo "head=$(git rev-parse HEAD)" >>"$out/run.meta"
echo "date=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"$out/run.meta"
echo "rust_bin=$rust_bin" >>"$out/run.meta"
echo "python_bin=$python_bin" >>"$out/run.meta"
echo "cargo=$($rust_bin/cargo --version)" >>"$out/run.meta"
echo "rustfmt=$($rust_bin/rustfmt --version)" >>"$out/run.meta"
echo "python=$($python_bin --version 2>&1)" >>"$out/run.meta"
echo "CARGO_NET_OFFLINE=$CARGO_NET_OFFLINE" >>"$out/run.meta"
echo "JCODE_NO_TELEMETRY=$JCODE_NO_TELEMETRY" >>"$out/run.meta"
echo "FORK_NUDGE_MAX_AGE=$FORK_NUDGE_MAX_AGE" >>"$out/run.meta"
echo "FORK_NUDGE_AUTOSYNC=$FORK_NUDGE_AUTOSYNC" >>"$out/run.meta"
echo "CARGO_TARGET_DIR=$CARGO_TARGET_DIR" >>"$out/run.meta"
echo "JCODE_HOME=$JCODE_HOME" >>"$out/run.meta"
echo "JCODE_RUNTIME_DIR=$JCODE_RUNTIME_DIR" >>"$out/run.meta"
echo "nix_invocations=none" >>"$out/run.meta"

process_snapshot() {
  local dest=$1
  {
    date -u +%Y-%m-%dT%H:%M:%SZ
    ps -axo pid,ppid,comm,args | awk '
      /nix develop|nix build|nix shell|nix run|nix-store|nix-build|ssh: \/tmp\/nix/ && $0 !~ /awk/ { print }
    '
  } >"$dest"
}

process_snapshot "$out/process_before.log"

run_green() {
  local name=$1
  shift
  echo "JCODE_PROGRESS {\"message\":\"Running $name\"}"
  set +e
  "$@" >"$out/$name.log" 2>&1
  local rc=$?
  set -e
  echo "$rc" >"$out/$name.exit"
  if [[ $rc -ne 0 ]]; then
    tail -120 "$out/$name.log"
    echo "FAIL $name expected=0 actual=$rc"
    exit 1
  fi
  echo "PASS $name exit=0"
}

run_red() {
  local name=$1
  shift
  echo "JCODE_PROGRESS {\"message\":\"Running expected-red $name\"}"
  set +e
  "$@" >"$out/$name.log" 2>&1
  local rc=$?
  set -e
  echo "$rc" >"$out/$name.exit"
  if [[ $rc -ne 1 ]]; then
    tail -120 "$out/$name.log"
    echo "FAIL $name expected=1 actual=$rc"
    exit 1
  fi
  echo "PASS $name expected-red exit=1"
}

cargo_test() {
  local filter=$1
  shift
  cargo test -p jcode-tui --lib "$filter" -- --nocapture --test-threads=1 "$@"
}

run_green fixture_timeout cargo_test import_review_timeout_fails_closed_without_import_task_transition
run_green fixture_escape_existing cargo_test liveness_esc_always_exits_onboarding_from_every_guided_phase
run_green fixture_decline_all_existing cargo_test liveness_import_review_decline_all_then_enter_escapes
run_green fixture_affirmative_existing cargo_test import_summary_defaults_to_continue_and_enter_imports_all

run_green affected_tui_check cargo check -p jcode-tui
run_green rustfmt_source_control rustfmt --edition 2024 --check crates/jcode-tui/src/tui/app/onboarding_flow_control.rs

run_green r09_classifier "$python_bin" -m unittest discover -s tests -p 'test_rust_production_filter.py'
run_green r09_dependency "$python_bin" scripts/check_dependency_boundaries.py
run_red r09_panic "$python_bin" scripts/check_panic_budget.py
run_red r09_swallowed "$python_bin" scripts/check_swallowed_error_budget.py
run_red r09_code_size "$python_bin" scripts/check_code_size_budget.py
run_red r09_test_size "$python_bin" scripts/check_test_size_budget.py
run_green r09_wildcard "$python_bin" scripts/check_wildcard_reexport_budget.py
run_green r09_warning bash scripts/check_warning_budget.sh
run_green r09_shell_syntax bash -n scripts/*.sh
run_green r09_diff_check git diff --check

process_snapshot "$out/process_after.log"

(
  cd "$out"
  shasum -a 256 *.log *.exit run.meta driver.sh > SHA256SUMS
)

echo 'JCODE_CHECKPOINT {"message":"W5 onboarding consent evidence passed all expected exits"}'
