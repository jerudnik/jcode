#!/usr/bin/env bash
# F23 non-vacuity harness: plant debt, observe the gate, revert, observe green.
# Not a committed gate; it produces docs/fork/ideal-base/evidence/F23/ output.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

OUT=/tmp/f23-proof
rm -rf "$OUT" && mkdir -p "$OUT"
CHECK="python3 scripts/check_critical_path_budget.py"

run() { # run <label> <cmd...>
  local label="$1"; shift
  echo "### $label" >> "$OUT/log.txt"
  echo "\$ $*" >> "$OUT/log.txt"
  "$@" >> "$OUT/log.txt" 2>&1
  local rc=$?
  echo "EXIT=$rc" >> "$OUT/log.txt"
  echo >> "$OUT/log.txt"
  echo "$label -> EXIT=$rc"
  return $rc
}

assert_clean() {
  if [ -n "$(git status --porcelain -- crates src scripts/*_budget.json scripts/warning_budget.txt)" ]; then
    echo "FATAL: tree not clean before/after plant" >&2
    git status --porcelain -- crates src scripts >&2
    exit 1
  fi
}

assert_clean

# --- CONTROL: green baseline -------------------------------------------------
run "control/green-baseline" $CHECK --report "$OUT/report-baseline.json"

# --- PLANT 1: new panic in a critical path (lifecycle) -----------------------
TARGET=crates/jcode-core/src/util.rs
cp "$TARGET" "$OUT/util.rs.orig"
cat >> "$TARGET" <<'EOF'

pub fn f23_planted_panic(v: Option<u32>) -> u32 {
    v.expect("F23 planted panic in a critical lifecycle path")
}
EOF
run "plant1/critical-panic-RED" $CHECK
cp "$OUT/util.rs.orig" "$TARGET"
run "plant1/reverted-GREEN" $CHECK
assert_clean

# --- PLANT 2: new swallowed error in a critical path (updater) ---------------
TARGET=crates/jcode-app-core/src/update.rs
cp "$TARGET" "$OUT/update.rs.orig"
cat >> "$TARGET" <<'EOF'

pub fn f23_planted_swallow() {
    let _ = std::fs::remove_file("/tmp/f23-planted-swallowed-error");
}
EOF
run "plant2/critical-swallow-RED" $CHECK
cp "$OUT/update.rs.orig" "$TARGET"
run "plant2/reverted-GREEN" $CHECK
assert_clean

# --- PLANT 3: new oversize file in a critical path (provider-infrastructure) --
TARGET=crates/jcode-provider-core/src/f23_planted_oversize.rs
{
  echo "// F23 planted oversize file in provider-infrastructure."
  for i in $(seq 1 1300); do echo "pub const F23_PLANTED_$i: u32 = $i;"; done
} > "$TARGET"
run "plant3/critical-oversize-RED" $CHECK
rm -f "$TARGET"
run "plant3/reverted-GREEN" $CHECK
assert_clean

# --- PLANT 4: same three defects OUTSIDE the critical scope -------------------
# Documented policy: this gate must stay green; the repository-wide ratchets are
# the ones that fail. jcode-fuzzy is a leaf crate in no critical domain.
TARGET=crates/jcode-fuzzy/src/lib.rs
cp "$TARGET" "$OUT/fuzzy-lib.rs.orig"
cat >> "$TARGET" <<'EOF'

pub fn f23_planted_noncritical(v: Option<u32>) -> u32 {
    let _ = v;
    v.expect("F23 planted panic outside the critical scope")
}
EOF
OVERSIZE=crates/jcode-fuzzy/src/f23_planted_oversize.rs
{
  echo "// F23 planted oversize file outside the critical scope."
  for i in $(seq 1 1300); do echo "pub const F23_PLANTED_$i: u32 = $i;"; done
} > "$OVERSIZE"
run "plant4/noncritical-critical-gate-GREEN" $CHECK
run "plant4/noncritical-repo-panic-ratchet-RED" python3 scripts/check_panic_budget.py
run "plant4/noncritical-repo-swallow-ratchet-RED" python3 scripts/check_swallowed_error_budget.py
run "plant4/noncritical-repo-codesize-ratchet-RED" python3 scripts/check_code_size_budget.py
cp "$OUT/fuzzy-lib.rs.orig" "$TARGET"
rm -f "$OVERSIZE"
run "plant4/reverted-critical-GREEN" $CHECK
run "plant4/reverted-repo-panic-GREEN" python3 scripts/check_panic_budget.py
assert_clean

# --- PLANT 5: repository trend raise (the gate the ratchets cannot enforce) ---
# Raise the unprotected panic baseline as a "cleanup accounting" edit would.
cp scripts/panic_budget.json "$OUT/panic_budget.json.orig"
python3 - <<'PY'
import json, pathlib
p = pathlib.Path("scripts/panic_budget.json")
d = json.loads(p.read_text())
d["total"] += 1
d["tracked_files"]["crates/jcode-fuzzy/src/lib.rs"] = 1
p.write_text(json.dumps(d, indent=2, sort_keys=True) + "\n")
PY
run "plant5/repo-trend-raise-RED" $CHECK
cp "$OUT/panic_budget.json.orig" scripts/panic_budget.json
run "plant5/reverted-GREEN" $CHECK
assert_clean

# --- PLANT 6: weakening the pinned block must break the CI digest pin ---------
PIN=$(grep -o '[0-9a-f]\{64\}' .github/workflows/fork-ci.yml | head -1)
echo "workflow pin: $PIN" >> "$OUT/log.txt"
run "plant6/pin-matches-baseline" $CHECK --expect-digest "$PIN"
cp scripts/check_critical_path_budget.py "$OUT/checker.py.orig"
python3 - <<'PY'
import pathlib
p = pathlib.Path("scripts/check_critical_path_budget.py")
s = p.read_text()
# A ceiling raise, exactly what a maintenance window must catch.
s = s.replace('"tui": {"panic": 8,', '"tui": {"panic": 99,')
p.write_text(s)
PY
run "plant6/ceiling-raise-breaks-pin-RED" $CHECK --expect-digest "$PIN"
run "plant6/ceiling-raise-hides-plant-without-pin" $CHECK
cp "$OUT/checker.py.orig" scripts/check_critical_path_budget.py
run "plant6/reverted-pin-GREEN" $CHECK --expect-digest "$PIN"

# --- PLANT 7: scope narrowing must break the pin ------------------------------
python3 - <<'PY'
import pathlib
p = pathlib.Path("scripts/check_critical_path_budget.py")
s = p.read_text()
s = s.replace('        "crates/jcode-tui/",\n', '')
p.write_text(s)
PY
run "plant7/scope-narrowing-breaks-pin-RED" $CHECK --expect-digest "$PIN"
cp "$OUT/checker.py.orig" scripts/check_critical_path_budget.py
run "plant7/reverted-pin-GREEN" $CHECK --expect-digest "$PIN"

# --- PLANT 8: oversize threshold drift in the unprotected baseline ------------
cp scripts/code_size_budget.json "$OUT/code_size_budget.json.orig"
python3 - <<'PY'
import json, pathlib
p = pathlib.Path("scripts/code_size_budget.json")
d = json.loads(p.read_text())
d["threshold_loc"] = 5000
p.write_text(json.dumps(d, indent=2, sort_keys=True) + "\n")
PY
run "plant8/threshold-drift-RED" $CHECK
cp "$OUT/code_size_budget.json.orig" scripts/code_size_budget.json
run "plant8/reverted-GREEN" $CHECK
assert_clean

echo
echo "FINAL git status:"
git status --porcelain
echo "log: $OUT/log.txt"
