#!/usr/bin/env bash
set -euo pipefail
repo=/Users/jrudnik/labs/jcode
out=${1:?}
cd "$repo"
export PATH=/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin:/usr/bin:/bin:/usr/sbin:/sbin
export CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 JCODE_NO_TELEMETRY=1 JCODE_TELEMETRY=0 JCODE_NUDGE_DISABLED=1 JCODE_SETUP_HINTS_DISABLED=1
export CARGO_TARGET_DIR=/Users/jrudnik/labs/jcode-w4-r02/target
export JCODE_HOME="$out/jcode-home" JCODE_RUNTIME_DIR="$out/jcode-runtime"
mkdir -p "$JCODE_HOME" "$JCODE_RUNTIME_DIR" "$out/raw"
manifest="$out/manifest.tsv"; : > "$manifest"
run_expect(){ local name=$1 expected=$2; shift 2; local log="$out/raw/$name.txt"; echo "JCODE_PROGRESS {\"message\":\"W4 integration $name\"}"; { echo "HEAD=$(git rev-parse HEAD)"; echo "PWD=$PWD"; echo "PATH=$PATH"; echo "CARGO_NET_OFFLINE=$CARGO_NET_OFFLINE"; echo "JCODE_HOME=$JCODE_HOME"; echo "JCODE_RUNTIME_DIR=$JCODE_RUNTIME_DIR"; echo "COMMAND: $*"; } >"$log"; set +e; "$@" >>"$log" 2>&1; rc=$?; set -e; printf '%s\t%s\t%s\t%s\n' "$name" "$expected" "$rc" "$*" >>"$manifest"; echo "EXIT: $rc" >>"$log"; [[ "$rc" == "$expected" ]] || { tail -120 "$log"; exit 1; }; }
process_cmd='import subprocess
text=subprocess.check_output(["ps","-axo","pid,ppid,comm,args"], text=True)
needles=("nix-daemon __build-remote","ssh john@10.201.0.7","ssh: /tmp/nix")
for line in text.splitlines():
    if any(n in line for n in needles) and "jcode-w4-integration-driver" not in line and "python3 -c" not in line:
        print(line)'
run_expect process_before 0 /usr/bin/python3 -c "$process_cmd"
run_expect exact_catalog_route_test 0 cargo test -p jcode-base --lib provider::catalog_routes::tests::openrouter_alternative_routes_skip_models_absent_from_catalog -- --exact --nocapture
run_expect exact_catalog_route_test_one_passed 0 bash -lc "grep -E 'test result: ok\\. 1 passed; 0 failed; 0 ignored; 0 measured; .* filtered out' '$out/raw/exact_catalog_route_test.txt'"
run_expect cargo_check_jcode_base 0 cargo check -p jcode-base
run_expect evidence_sha256 0 bash -lc 'cd docs/fork/recovery/evidence/2026-07-16-w4-r02-route-closure && shasum -a 256 -c SHA256SUMS'
run_expect zero_rust_diff 0 bash -lc 'test -z "$(git diff --name-only 3c7a0d0500c400e64136f51f6746674235021427..HEAD -- "*.rs")"'
run_expect integrated_paths 1 bash -lc "git diff --name-only 3c7a0d0500c400e64136f51f6746674235021427..HEAD | grep -Ev '^(docs/fork/recovery/seams/R02-config-provider-routing/ledger.md|docs/fork/recovery/evidence/2026-07-16-w4-r02-route-closure/)'"
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
run_expect prompt_hash 0 bash -lc 'test "$(git diff -- docs/fork/recovery/ORCHESTRATOR_PROMPT.md | shasum -a 256 | cut -d " " -f1)" = 8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00'
run_expect stash_count 0 bash -lc 'test "$(git stash list | wc -l | tr -d " ")" = 4'
run_expect process_after 0 /usr/bin/python3 -c "$process_cmd"
run_expect process_equal 0 bash -lc "diff -u <(tail -n +8 '$out/raw/process_before.txt' | sed '\$d') <(tail -n +8 '$out/raw/process_after.txt' | sed '\$d')"
run_expect no_update_invocation 0 /usr/bin/python3 -c 'import pathlib,sys; root=pathlib.Path(sys.argv[1]); needle="-"+"-update"; bad=[]
for p in [root/"manifest.tsv", pathlib.Path("/tmp/jcode-w4-integration-driver-v2.sh")]:
 t=p.read_text(errors="replace").replace("needle=\"-\"+\"-update\"", "needle=<constructed>")
 if needle in t: bad.append(str(p))
print("hits",len(bad)); print("\n".join(bad)); raise SystemExit(bool(bad))' "$out"
run_expect final_status 0 git status --short
printf 'JCODE_CHECKPOINT {"message":"W4 post-integration validation matched all expected exits"}\n'
