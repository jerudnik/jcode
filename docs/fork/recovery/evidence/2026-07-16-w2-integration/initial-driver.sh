#!/usr/bin/env bash
set -euo pipefail
repo=/Users/jrudnik/labs/jcode
out=/tmp/jcode-w2-post-integration
mkdir -p "$out"
cd "$repo"
export CARGO_NET_OFFLINE=true
export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR=/tmp/jcode-w2-post-integration-target
export JCODE_NO_TELEMETRY=1

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
    tail -80 "$out/$name.log"
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
    tail -80 "$out/$name.log"
    echo "FAIL $name expected=1 actual=$rc"
    exit 1
  fi
  echo "PASS $name expected-red exit=1"
}

commands=(
  "handle_comm_spawn_auto_fallback_preserves_history_and_detail_with_prompt"
  "communicate_run_plan_churns_to_abort_at_configured_concurrency_and_cleans_failed_workers"
  "visible_launch"
  "assign_task_stale_direct_takeover_preserves_progress_history"
  "reclaim_stranded_assignment_releases_owner_and_counts_reclaims"
  "salvage_requeues_dead_members_tasks_and_notifies_coordinator"
  "salvage_fails_task_once_reclaim_cap_is_reached"
  "member_status_is_dead_matches_terminal_non_success_states"
  "f1_assign_next_reclaims_task_from_departed_assignee"
  "failed_instance_needs_retry"
  "dead_pid_sweep_then_salvage_requeues_once_without_duplicate_assignment"
  "control_log_fold_tracks_maps_through_handler_sequence"
  "scan_from_tail_offset_finds_artifact_once"
)

for i in "${!commands[@]}"; do
  name=${commands[$i]}
  package=jcode-app-core
  if [[ $name == reclaim_stranded_assignment_releases_owner_and_counts_reclaims ]]; then
    package=jcode-plan
  fi
  echo "JCODE_PROGRESS {\"current\":$((i+1)),\"total\":13,\"unit\":\"fixtures\",\"message\":\"$name\"}"
  run_green "fixture_$((i+1))_$name" bash scripts/dev_cargo.sh test -p "$package" "$name" -- --nocapture --test-threads=1
done

run_green package_check bash scripts/dev_cargo.sh check -p jcode-app-core -p jcode-protocol -p jcode-plan
run_green classifier python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'
run_green dependency python3 scripts/check_dependency_boundaries.py
run_red panic python3 scripts/check_panic_budget.py
run_red swallowed python3 scripts/check_swallowed_error_budget.py
run_red code_size python3 scripts/check_code_size_budget.py
run_red test_size python3 scripts/check_test_size_budget.py
run_green wildcard python3 scripts/check_wildcard_reexport_budget.py
run_green warning bash scripts/check_warning_budget.sh
run_green shell_syntax bash -n scripts/*.sh
run_green diff_check git diff --check

printf 'JCODE_CHECKPOINT {"message":"W2 post-integration fixtures and R09 gates matched all expected exits"}\n'
