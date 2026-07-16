#!/usr/bin/env bash
set -euo pipefail
repo=/Users/jrudnik/labs/jcode-w4-r02
out=${1:?out dir}
cd "$repo"
export PATH="/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export CARGO_NET_OFFLINE=true
export CARGO_INCREMENTAL=0
export JCODE_HOME="$out/jcode-home"
export JCODE_RUNTIME_DIR="$out/jcode-runtime"
export JCODE_NO_TELEMETRY=1
export JCODE_TELEMETRY=0
export JCODE_NUDGE_DISABLED=1
export JCODE_SETUP_HINTS_DISABLED=1
mkdir -p "$JCODE_HOME" "$JCODE_RUNTIME_DIR" "$out/raw"
manifest="$out/manifest.tsv"
: > "$manifest"
run_expect() {
  local name=$1 expected=$2; shift 2
  local log="$out/raw/${name}.txt"
  echo "JCODE_PROGRESS {\"message\":\"$name\"}"
  {
    echo "PWD=$PWD"
    echo "PATH=$PATH"
    echo "JCODE_HOME=$JCODE_HOME"
    echo "JCODE_RUNTIME_DIR=$JCODE_RUNTIME_DIR"
    echo "CARGO_NET_OFFLINE=$CARGO_NET_OFFLINE"
    echo "COMMAND: $*"
  } > "$log"
  set +e
  "$@" >> "$log" 2>&1
  local rc=$?
  set -e
  printf '%s\t%s\t%s\t%s\n' "$name" "$expected" "$rc" "$*" >> "$manifest"
  echo "EXIT: $rc" >> "$log"
  if [[ "$rc" != "$expected" ]]; then
    tail -120 "$log"
    echo "FAIL $name expected=$expected actual=$rc"
    exit 1
  fi
  echo "PASS $name expected=$expected actual=$rc"
}
run_expect rust_toolchain 0 bash -lc 'cargo --version && rustc --version && rustc --print sysroot'
run_expect pre_status 0 git status --short
run_expect no_rust_diff_before 0 bash -lc 'test -z "$(git diff -- crates/**/*.rs crates/*.rs 2>/dev/null)"'
run_expect exact_catalog_route_test 0 cargo test -p jcode-base --lib provider::catalog_routes::tests::openrouter_alternative_routes_skip_models_absent_from_catalog -- --exact --nocapture
run_expect exact_catalog_route_test_one_passed 0 bash -lc "grep -E 'test result: ok\\. 1 passed; 0 failed; 0 ignored; 0 measured; .* filtered out' '$out/raw/exact_catalog_route_test.txt'"
run_expect cargo_check_jcode_base 0 cargo check -p jcode-base
run_expect no_rust_diff_after 0 bash -lc 'test -z "$(git diff -- crates/**/*.rs crates/*.rs 2>/dev/null)"'
run_expect changed_paths_allowed 1 bash -lc "git diff --name-only | grep -Ev '^(docs/fork/recovery/seams/R02-config-provider-routing/ledger.md|docs/fork/recovery/evidence/2026-07-16-w4-r02-route-closure/)'"
run_expect classifier 0 /usr/bin/python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'
run_expect dependency 0 /usr/bin/python3 scripts/check_dependency_boundaries.py
run_expect panic 1 /usr/bin/python3 scripts/check_panic_budget.py
run_expect swallowed 1 /usr/bin/python3 scripts/check_swallowed_error_budget.py
run_expect code_size 1 /usr/bin/python3 scripts/check_code_size_budget.py
run_expect test_size 1 /usr/bin/python3 scripts/check_test_size_budget.py
run_expect wildcard 0 /usr/bin/python3 scripts/check_wildcard_reexport_budget.py
run_expect warning 0 bash scripts/check_warning_budget.sh
run_expect shell_syntax 0 bash -n scripts/*.sh
run_expect diff_check 0 git diff --check
run_expect no_update_invocation_guard 0 /usr/bin/python3 -c 'import pathlib,sys; root=pathlib.Path(sys.argv[1]); needle="-"+"-update"; bad=[]
for p in [root/"manifest.tsv", root/"run-validation.sh"]:
    text=p.read_text(errors="replace")
    # Allow this guard to construct/search the forbidden token without invoking it.
    cleaned=text.replace("needle=\"-\"+\"-update\"", "needle=<constructed>")
    if needle in cleaned:
        bad.append(str(p))
print("NO_UPDATE_INVOCATION_HITS", len(bad))
print("\n".join(bad))
sys.exit(1 if bad else 0)' "$out"
run_expect final_status 0 git status --short
printf 'JCODE_CHECKPOINT {"message":"W4 R02 accepted validation matched expected exits including final_status"}\n'
