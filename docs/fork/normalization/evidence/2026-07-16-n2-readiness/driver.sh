#!/usr/bin/env bash
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
out=${1:?usage: driver.sh OUTPUT_DIR}
cd "$repo"

export CARGO_NET_OFFLINE=true
export CARGO_INCREMENTAL=0
export JCODE_NO_TELEMETRY=1
export JCODE_TELEMETRY=0
export JCODE_SETUP_HINTS_DISABLED=1
export FORK_NUDGE_MAX_AGE=2147483647
export FORK_NUDGE_AUTOSYNC=0
export JCODE_HOME="$out/jcode-home"
export JCODE_RUNTIME_DIR="$out/jcode-runtime"
mkdir -p "$JCODE_HOME" "$JCODE_RUNTIME_DIR" "$out/raw" "$out/homes"
manifest="$out/manifest.tsv"
: > "$manifest"

run_expect() {
  local name=$1 expected=$2
  shift 2
  local log="$out/raw/$name.txt"
  printf 'JCODE_PROGRESS {"message":"N2 trusted matrix: %s"}\n' "$name"
  {
    echo "HEAD=$(git rev-parse HEAD)"
    echo "PWD=$PWD"
    echo "CARGO_NET_OFFLINE=$CARGO_NET_OFFLINE"
    echo "JCODE_HOME=$JCODE_HOME"
    echo "JCODE_RUNTIME_DIR=$JCODE_RUNTIME_DIR"
    printf 'COMMAND:'
    printf ' %q' "$@"
    printf '\n'
  } > "$log"
  set +e
  "$@" >> "$log" 2>&1
  local rc=$?
  set -e
  printf '%s\t%s\t%s\t' "$name" "$expected" "$rc" >> "$manifest"
  printf '%q ' "$@" >> "$manifest"
  printf '\n' >> "$manifest"
  echo "EXIT: $rc" >> "$log"
  if [[ "$rc" != "$expected" ]]; then
    tail -200 "$log"
    return 1
  fi
}

run_expect clean_start 0 bash -lc 'test -z "$(git status --short)"'
run_expect branch 0 bash -lc 'test "$(git branch --show-current)" = normalize/integration'
run_expect main_ancestor 0 bash -lc 'test "$(git merge-base main HEAD)" = "$(git rev-parse main)"'
run_expect recovery_archive 0 git cat-file -e refs/archive/recovery/2026-07-15^{commit}
run_expect recovery_product_source_equivalent 0 bash -lc \
  'test -z "$(git diff --name-only 51168d16e9c708ae4afff09a6fc6402642d17782 refs/archive/recovery/2026-07-15 -- . ":(exclude)docs/fork")"'

cat > "$out/expected-product-diff-paths.txt" <<'PATHS'
crates/jcode-app-core/src/agent/evidence.rs
crates/jcode-app-core/src/agent/turn_loops.rs
crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs
crates/jcode-app-core/src/build.rs
crates/jcode-app-core/src/server/client_actions.rs
crates/jcode-app-core/src/server/client_lifecycle.rs
crates/jcode-app-core/src/server/handshake.rs
crates/jcode-app-core/src/server/swarm.rs
crates/jcode-base/src/subscription_api.rs
crates/jcode-build-support/src/tests.rs
crates/jcode-plan/src/lib.rs
crates/jcode-storage/src/active_pids.rs
src/cli/terminal.rs
PATHS
git diff --name-only refs/archive/recovery/2026-07-15 HEAD -- . ':(exclude)docs/fork' \
  | sort > "$out/actual-product-diff-paths.txt"
run_expect enumerated_product_diff 0 diff -u \
  "$out/expected-product-diff-paths.txt" "$out/actual-product-diff-paths.txt"
run_expect frozen_quality_baselines 0 git diff --exit-code \
  refs/archive/recovery/2026-07-15 HEAD -- \
  scripts/panic_budget.json scripts/swallowed_error_budget.json \
  scripts/code_size_budget.json scripts/test_size_budget.json
run_expect baseline_hashes 0 bash -lc \
  'printf "%s  %s\n" \
    aaa2b72dff641c482248676b4b1309cc98fd21934370eec74fb62ee9579cece8 scripts/panic_budget.json \
    0b70750a82c17771726cacbe6deeefe807b09058d2e0078d1dfc2c31e8b53dc8 scripts/swallowed_error_budget.json \
    c7e46062390fa73d3ebfd99217bda289290ab146a104a2d8d501a72bc0c6cd19 scripts/code_size_budget.json \
    5402e55b096d1b6bb71ea0bc38c39fa3eda98c22290e7d863418d52508997ee6 scripts/test_size_budget.json \
    | sha256sum -c -'
run_expect protocol_version 0 grep -Eq 'pub const PROTOCOL_VERSION: u32 = 1;' \
  crates/jcode-protocol/src/lib.rs

run_expect rustfmt 0 cargo fmt --all -- --check
run_expect classifier 0 /usr/bin/python3 -m unittest discover -s tests -p test_rust_production_filter.py
run_expect classifier_compile 0 /usr/bin/python3 -m py_compile \
  scripts/rust_production_filter.py scripts/check_panic_budget.py \
  scripts/check_swallowed_error_budget.py tests/test_rust_production_filter.py
run_expect dependency 0 /usr/bin/python3 scripts/check_dependency_boundaries.py
run_expect wildcard 0 /usr/bin/python3 scripts/check_wildcard_reexport_budget.py
run_expect warning 0 bash scripts/check_warning_budget.sh
run_expect shell_syntax 0 bash -n scripts/*.sh
run_expect diff_check 0 git diff --check

run_expect build_support_suite 0 cargo test -p jcode-build-support --lib -- --nocapture --test-threads=1
run_expect protocol_suite 0 cargo test -p jcode-protocol --lib -- --nocapture --test-threads=1
run_expect r02_subscription_suite 0 cargo test -p jcode-base --lib subscription -- --nocapture --test-threads=1
run_expect r02_provider_filters 0 cargo test -p jcode-base --lib provider::tests::test_subscription_ -- --nocapture --test-threads=1

cat > "$out/r04-fixtures.tsv" <<'MATRIX'
orphan_reload	jcode-base	background::tests::reconcile_marks_orphan_from_reloaded_process_failed
post_reload_cancel	jcode-app-core	server::client_lifecycle::tests::cancel_aborts_detached_streaming_turn_with_stale_stop_signal
graceful_initiator	jcode-app-core	server::reload::reload_tests::graceful_shutdown_sessions_signals_all_running_sessions_including_initiator
graceful_partial	jcode-app-core	server::reload::reload_tests::graceful_shutdown_sessions_times_out_on_partial_checkpoint
wait_evidence	jcode-app-core	agent::turn_streaming_mpsc::tests::reload_interrupted_bg_wait_is_interrupted_and_resumable
wait_render	jcode-app-core	server::client_session::tests::reload_tests::detects_resumable_reload_interrupted_wait_with_error_bit
recovery_exact_once	jcode-app-core	server::client_state::client_state_tests::history_reload_recovery_does_not_mark_delivered_until_continuation_is_accepted
restart_identity	jcode-app-core	server::reload_state::tests::restart_identity_projection_carries_dirty_same_commit_without_reclassification
marker_combinations	jcode-storage	active_pids::tests::conditional_session_marker_cleanup_reports_exact_partial_removals
marker_lock_bound	jcode-storage	active_pids::tests::held_marker_lock_is_bounded_and_fail_closed_without_sleeping
terminal_persisted	jcode-app-core	server::client_disconnect_cleanup::tests::idle_closed_disconnect_persists_closed_before_preserving_successor_marker
terminal_not_required	jcode-app-core	server::client_disconnect_cleanup::tests::successor_connected_cleanup_reports_terminal_not_required
terminal_failed	jcode-app-core	server::client_disconnect_cleanup::tests::crashed_disconnect_save_failure_retains_successor_marker_and_cleans_runtime_state
terminal_lock_timeout	jcode-app-core	server::client_disconnect_cleanup::tests::disconnect_agent_lock_timeout_is_observable_without_terminal_persistence
MATRIX
while IFS=$'\t' read -r label package test_name; do
  home="$out/homes/r04-$label"
  mkdir -p "$home/home" "$home/runtime"
  run_expect "r04_$label" 0 env JCODE_HOME="$home/home" \
    JCODE_RUNTIME_DIR="$home/runtime" cargo test -p "$package" --lib \
    "$test_name" -- --exact --nocapture --test-threads=1
done < "$out/r04-fixtures.tsv"

r12_home="$out/homes/r12"
mkdir -p "$r12_home/home" "$r12_home/runtime"
run_expect r12_suite 0 env JCODE_HOME="$r12_home/home" \
  JCODE_RUNTIME_DIR="$r12_home/runtime" cargo test -p jcode-app-core --lib \
  r12_ -- --nocapture --test-threads=1
run_expect w7_provenance 0 cargo test -p jcode-plan provenance_ -- --nocapture --test-threads=1
run_expect app_core_lib 0 cargo test -p jcode-app-core --lib -- --test-threads=1
run_expect base_lib 0 cargo test -p jcode-base --lib -- --test-threads=1
run_expect storage_lib 0 cargo test -p jcode-storage --lib -- --test-threads=1
run_expect tui_lib 0 cargo test -p jcode-tui --lib -- --test-threads=1
run_expect workspace_tests 0 cargo test --workspace -- --test-threads=1
run_expect workspace_check 0 cargo check --workspace
run_expect workspace_clippy 0 cargo clippy --workspace --all-targets -- -D warnings
run_expect tui_build 0 cargo build -p jcode --bin jcode
run_expect binary_version 0 target/debug/jcode --version

run_expect panic 1 /usr/bin/python3 scripts/check_panic_budget.py
run_expect panic_exact 0 grep -F '31 -> 48' "$out/raw/panic.txt"
run_expect swallowed 1 /usr/bin/python3 scripts/check_swallowed_error_budget.py
run_expect swallowed_exact 0 grep -F '2987 -> 3074' "$out/raw/swallowed.txt"
run_expect code_size 1 /usr/bin/python3 scripts/check_code_size_budget.py
run_expect test_size 1 /usr/bin/python3 scripts/check_test_size_budget.py

run_expect no_update_invocation 0 /usr/bin/python3 -c \
  'import pathlib,sys; p=pathlib.Path(sys.argv[1]); needle="-"+"-update"; text=p.read_text(); text=text.replace("needle=\"-\"+\"-update\"", "needle=<constructed>"); raise SystemExit(needle in text)' \
  "$0"
run_expect final_status 0 bash -lc 'test -z "$(git status --short)"'
sha256sum "$manifest" "$out"/raw/*.txt > "$out/SHA256SUMS"
printf 'JCODE_CHECKPOINT {"message":"N2 trusted matrix passed"}\n'
