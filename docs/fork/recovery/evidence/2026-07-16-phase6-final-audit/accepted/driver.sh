#!/usr/bin/env bash
set -euo pipefail
repo=/Users/jrudnik/labs/jcode
out=${1:?}
cd "$repo"
export PATH=/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin:/usr/bin:/bin:/usr/sbin:/sbin
export CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 JCODE_NO_TELEMETRY=1 JCODE_TELEMETRY=0 FORK_NUDGE_MAX_AGE=2147483647 FORK_NUDGE_AUTOSYNC=0 JCODE_SETUP_HINTS_DISABLED=1
export CARGO_TARGET_DIR=/Users/jrudnik/labs/jcode-w3-r04/target
export JCODE_HOME="$out/jcode-home" JCODE_RUNTIME_DIR="$out/jcode-runtime"
mkdir -p "$JCODE_HOME" "$JCODE_RUNTIME_DIR" "$out/raw" "$out/homes"
manifest="$out/manifest.tsv"; : > "$manifest"
run_expect(){ local name=$1 expected=$2; shift 2; local log="$out/raw/$name.txt"; echo "JCODE_PROGRESS {\"message\":\"Phase 6 final audit $name\"}"; { echo "HEAD=$(git rev-parse HEAD)"; echo "PWD=$PWD"; echo "PATH=$PATH"; echo "CARGO_NET_OFFLINE=$CARGO_NET_OFFLINE"; echo "JCODE_HOME=$JCODE_HOME"; echo "JCODE_RUNTIME_DIR=$JCODE_RUNTIME_DIR"; echo "COMMAND: $*"; } >"$log"; set +e; "$@" >>"$log" 2>&1; rc=$?; set -e; printf '%s\t%s\t%s\t%s\n' "$name" "$expected" "$rc" "$*" >>"$manifest"; echo "EXIT: $rc" >>"$log"; [[ "$rc" == "$expected" ]] || { tail -160 "$log"; exit 1; }; }
process_cmd='import subprocess
text=subprocess.check_output(["ps","-axo","pid,ppid,comm,args"], text=True)
needles=("nix-daemon __build-remote","ssh john@10.201.0.7","ssh: /tmp/nix")
for line in text.splitlines():
    if any(n in line for n in needles) and "jcode-phase6-final-audit-driver" not in line and "python3 -c" not in line:
        print(line)'
run_expect process_before 0 /usr/bin/python3 -c "$process_cmd"
run_expect branch 0 bash -lc 'test "$(git branch --show-current)" = recovery/2026-07-15'
run_expect vendor_upstream_pin 0 bash -lc 'test "$(git rev-parse vendor/upstream)" = 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d'
run_expect merge_base 0 bash -lc 'test "$(git merge-base 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d HEAD)" = 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d'
run_expect prompt_hash 0 bash -lc 'test "$(git diff -- docs/fork/recovery/ORCHESTRATOR_PROMPT.md | shasum -a 256 | cut -d " " -f1)" = 8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00'
run_expect sole_dirty_prompt 0 bash -lc 'test "$(git status --short)" = " M docs/fork/recovery/ORCHESTRATOR_PROMPT.md"'
run_expect stash_count 0 bash -lc 'test "$(git stash list | wc -l | tr -d " ")" = 4'
run_expect protocol_version 0 bash -lc 'grep -Eq "pub const PROTOCOL_VERSION: u32 = 1;" crates/jcode-protocol/src/lib.rs'
run_expect build_support_suite 0 cargo test -p jcode-build-support --lib -- --nocapture --test-threads=1
run_expect build_support_count 0 bash -lc "grep -E 'test result: ok\\. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' '$out/raw/build_support_suite.txt'"
run_expect protocol_suite 0 cargo test -p jcode-protocol --lib -- --nocapture --test-threads=1
run_expect protocol_count 0 bash -lc "grep -E 'test result: ok\\. 81 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' '$out/raw/protocol_suite.txt'"
run_expect r02_subscription_suite 0 cargo test -p jcode-base --lib subscription -- --nocapture --test-threads=1
run_expect r02_subscription_count 0 bash -lc "grep -E 'test result: ok\\. 38 passed; 0 failed; 0 ignored; 0 measured; .* filtered out' '$out/raw/r02_subscription_suite.txt'"
run_expect r02_provider_filters 0 cargo test -p jcode-base --lib provider::tests::test_subscription_ -- --nocapture --test-threads=1
run_expect r02_provider_count 0 bash -lc "grep -E 'test result: ok\\. 4 passed; 0 failed; 0 ignored; 0 measured; .* filtered out' '$out/raw/r02_provider_filters.txt'"
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
  home="$out/homes/r04-$label"; mkdir -p "$home/home" "$home/runtime"
  run_expect "r04_$label" 0 env JCODE_HOME="$home/home" JCODE_RUNTIME_DIR="$home/runtime" cargo test -p "$package" --lib "$test_name" -- --exact --nocapture --test-threads=1
  run_expect "r04_${label}_count" 0 bash -lc "grep -E 'test result: ok\\. 1 passed; 0 failed; 0 ignored; 0 measured; .* filtered out' '$out/raw/r04_$label.txt'"
done < "$out/r04-fixtures.tsv"
r12_home="$out/homes/r12"; mkdir -p "$r12_home/home" "$r12_home/runtime"
run_expect r12_suite 0 env JCODE_HOME="$r12_home/home" JCODE_RUNTIME_DIR="$r12_home/runtime" cargo test -p jcode-app-core --lib r12_ -- --nocapture --test-threads=1
run_expect r12_count 0 bash -lc "grep -E 'test result: ok\\. 11 passed; 0 failed; 0 ignored; 0 measured; .* filtered out' '$out/raw/r12_suite.txt'"
run_expect affected_checks 0 cargo check -p jcode-build-support -p jcode-protocol -p jcode-base -p jcode-app-core -p jcode-storage -p jcode-tui
run_expect classifier 0 /usr/bin/python3 -m unittest discover -s tests -p test_rust_production_filter.py
run_expect dependency 0 /usr/bin/python3 scripts/check_dependency_boundaries.py
run_expect panic 1 /usr/bin/python3 scripts/check_panic_budget.py
run_expect swallowed 1 /usr/bin/python3 scripts/check_swallowed_error_budget.py
run_expect code_size 1 /usr/bin/python3 scripts/check_code_size_budget.py
run_expect test_size 1 /usr/bin/python3 scripts/check_test_size_budget.py
run_expect wildcard 0 /usr/bin/python3 scripts/check_wildcard_reexport_budget.py
run_expect warning 0 bash scripts/check_warning_budget.sh
run_expect shell_syntax 0 bash -n scripts/*.sh
run_expect diff_check 0 git diff --check
run_expect no_active_build 0 bash -lc "! ps -axo command= | grep -E 'cargo (build|check|test)|rustc|selfdev.*(build|test)|nix-daemon __build-remote|ssh john@10\\.201\\.0\\.7' | grep -vE 'grep|jcode-phase6-final-audit-driver'"
run_expect process_after 0 /usr/bin/python3 -c "$process_cmd"
run_expect process_equal 0 bash -lc "diff -u <(tail -n +8 '$out/raw/process_before.txt' | sed '\$d') <(tail -n +8 '$out/raw/process_after.txt' | sed '\$d')"
run_expect no_update_invocation 0 /usr/bin/python3 -c 'import pathlib,sys; root=pathlib.Path(sys.argv[1]); needle="-"+"-update"; bad=[]
for p in [root/"manifest.tsv", pathlib.Path("/tmp/jcode-phase6-final-audit-driver.sh")]:
 t=p.read_text(errors="replace").replace("needle=\"-\"+\"-update\"", "needle=<constructed>")
 if needle in t: bad.append(str(p))
print("hits",len(bad)); print("\n".join(bad)); raise SystemExit(bool(bad))' "$out"
run_expect final_status 0 git status --short
printf 'JCODE_CHECKPOINT {"message":"Phase 6 combined cross-seam and preservation audit passed"}\n'
