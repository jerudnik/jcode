#!/usr/bin/env bash
# D02 landing scoreboard.
#
# Collapses the D02 maintenance-window work to five integers plus one string
# equality, so progress is one rerunnable reading rather than a prose claim.
# Exit 0 only when every engineering-side integer is at its required value.
#
# The window itself (governance write) is deliberately NOT scored here: it is
# blocked on user authorization, and folding it into an engineering scoreboard
# would misreport "waiting on a human" as "work unfinished". It is reported
# separately at the bottom as state, not as a failing check.
#
# Usage: ./scripts/d02_scoreboard.sh
set -uo pipefail

export PATH="/etc/profiles/per-user/${USER}/bin:/nix/var/nix/profiles/default/bin:${PATH}"
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 2

fail=0
note() { printf '%-34s %s\n' "$1" "$2"; }
check() { # name actual required
  if [[ "$2" == "$3" ]]; then
    printf '  ok   %-30s %s\n' "$1" "$2"
  else
    printf '  FAIL %-30s %s (want %s)\n' "$1" "$2" "$3"
    fail=$((fail + 1))
  fi
}

echo "D02 scoreboard"
echo

# (1) test suite: exit status read directly, never through a pipe.
python3 -m unittest tests.test_ideal_base_railway >/tmp/d02_sb_tests.txt 2>&1
tests_exit=$?
ran=$(grep -Eo '^Ran [0-9]+' /tmp/d02_sb_tests.txt | grep -Eo '[0-9]+' || echo 0)
check "tests_exit" "$tests_exit" "0"
check "tests_ran" "$ran" "27"

# (2) guard FIRES. Asserted functionally, not by grepping for its message: a
# grep proves a string exists, and the first version of this check returned 0
# because the message is line-wrapped. Worse, a reverted guard whose tests were
# reverted with it would leave (1) green, so the behaviour is driven directly
# with a minimal synthetic graph. Also asserts the guard stays SILENT on the
# same graph with the child complete, which is the acceptance-side control: a
# guard that flagged everything would pass a fires-only check.
python3 - <<'PY'
import sys
sys.path.insert(0, "scripts")
from ideal_base_railway import expansion_violations

graph = {"expansions": {"R": [{"id": "C"}]}}
def st(child):
    return {"nodes": {"R": {"state": "accepted"}, "C": {"state": child}}}

assert expansion_violations(graph, st("pending")), \
    "guard did NOT fire on a complete root over a pending child"
assert not expansion_violations(graph, st("accepted")), \
    "guard fired on a fully complete wave (would flag everything)"
PY
check "guard_fires_and_is_quiet" "$?" "0"

# (3) inertness on the live tree: this guard forbids a state main must not
# already be in, so check must exit 0 both before and after the merge.
python3 scripts/ideal_base_railway.py check \
  --published-ref refs/remotes/github/main >/tmp/d02_sb_check.txt 2>&1
check "railway_check_exit" "$?" "0"

# (4) protected-path prediction. An empty pattern parse is an ARTIFACT, not a
# clean bill of health, so the parse asserts non-emptiness before the hit count
# is trusted. 2 hits is the whole reason a window is required.
read -r pats hits < <(python3 - <<'PY'
import fnmatch, re, subprocess
wf = open(".github/workflows/governance-root.yml").read()
m = re.search(r'protected=\(\s*(.*?)\s*\)', wf, re.S)
assert m, "protected=( ... ) array not found"
pats = [p.strip() for p in m.group(1).split()
        if p.strip() and not p.strip().startswith('#')]
assert pats, "EMPTY PATTERN SET is an artifact, not a clean bill of health"
changed = subprocess.run(["git", "diff", "--name-only", "github/main..HEAD"],
                         capture_output=True, text=True, check=True).stdout.split()
hits = [f for f in changed
        if any(fnmatch.fnmatch(f, p) or f == p for p in pats)]
print(len(pats), len(hits))
PY
)
check "protected_patterns" "$pats" "32"
check "protected_hits" "$hits" "2"

echo
echo "state (not scored: blocked on authorization, not on engineering)"
note "commits not in main" "$(git rev-list --count github/main..HEAD)"
# unpushed is a separate fact from unlanded: work can be fully pushed to the
# PR branch and still be 5 commits ahead of main. Reporting one number as both
# is how a pushed branch reads as "nothing sent yet".
_up="$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null || true)"
if [[ -n "$_up" ]]; then
  note "unpushed to $_up" "$(git rev-list --count "$_up"..HEAD)"
else
  note "unpushed" "no upstream set for $(git rev-parse --abbrev-ref HEAD)"
fi
note "working tree" "$(git status --porcelain | wc -l | tr -d ' ') dirty files"
note "pre-window governed hash" "43ba61a7a5...94f2b (must match post-restore)"
note "identity asserts captured at" "$(cat /tmp/gwin/expected_base_sha.txt 2>/dev/null || echo 'unknown; re-run preflight')"
note "current github/main" "$(git rev-parse github/main)"
if [[ "$(cat /tmp/gwin/expected_base_sha.txt 2>/dev/null)" != "$(git rev-parse github/main)" ]]; then
  note "STALENESS" "main moved; re-run identity asserts before any write"
fi

echo
if [[ $fail -eq 0 ]]; then
  echo "ENGINEERING: green ($fail failing). Remaining work is the authorized window."
else
  echo "ENGINEERING: $fail failing."
fi
exit "$fail"
