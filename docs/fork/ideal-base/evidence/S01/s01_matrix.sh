#!/usr/bin/env bash
# S01: deterministic ideal-base matrix, ONE round.
#
# Run twice at the same commit from clean runtime state; compare the two
# normalized transcript hashes. See NORMALIZER_SPEC.md (frozen before first
# use) for what may and may not be erased before hashing.
#
# Usage: s01_matrix.sh <round-label>
# Writes: docs/fork/ideal-base/evidence/S01/round-<label>.log
#
# PRE-WARM NOTE (decided before the first round, recorded here):
# A cold cargo cache makes round 1 emit "Compiling ..." lines that a warm
# round 2 does not. That is a fixture difference, not a determinism finding.
# It is handled by pre-warming the build BEFORE both rounds (see prewarm.sh),
# never by widening the normalizer after observing a hash disagreement.
#
# BUILD LOCUS PIN (JCODE_REMOTE_CARGO=0), decided before the first round:
# scripts/dev_cargo.sh routes cargo to a remote builder when
# JCODE_REMOTE_CARGO=1, which is read from ~/.config/jcode/remote-build.env,
# a machine-local file OUTSIDE this repository and outside its history. Two
# rounds run under different values of that file are not two rounds of the
# same experiment, and no second party could reproduce H. The locus is
# therefore pinned to local here.
#
# This pin narrows no gate: all 9 steps still run, unmodified. It fixes WHERE
# they run. F14's accepted 18 PASS / 0 FAIL baseline was itself a local run
# (its transcript contains no remote markers), so pinning local reproduces
# F14's conditions rather than relaxing them.
#
# The pin also masks a real defect, which is why it is NOT the whole response:
# remote_build.sh rsyncs with --exclude '.git', and is_jcode_repo() requires a
# .git entry, so get_repo_dir() returns None on the remote and the selfdev
# reload path fails there. That is recorded as its own finding in FINDINGS.md
# and is not silently absorbed by this pin.
export JCODE_REMOTE_CARGO=0

set -uo pipefail

LABEL="${1:?usage: s01_matrix.sh <round-label>}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
EVID="$REPO/docs/fork/ideal-base/evidence/S01"
LOG="$EVID/round-$LABEL.log"
FAIL=0
NSTEP=0

export PATH="$HOME/.cargo/bin:$HOME/.nix-profile/bin:/etc/profiles/per-user/$USER/bin:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
export ANTHROPIC_API_KEY="" OPENAI_API_KEY="" GEMINI_API_KEY="" \
       OPENROUTER_API_KEY="" GROQ_API_KEY="" JCODE_NO_TELEMETRY=1

log() { printf '[%s] %s\n' "$(date -u '+%H:%M:%S')" "$*" | tee -a "$LOG"; }

run_step() {
    local label="$1"; shift
    NSTEP=$((NSTEP + 1))
    if "$@" >> "$LOG" 2>&1; then
        log "PASS $label"
    else
        local rc=$?
        log "FAIL $label (exit $rc)"
        FAIL=$((FAIL + 1))
    fi
}

: > "$LOG"
cd "$REPO"
log "S01 round=$LABEL commit=$(git rev-parse HEAD)"
log "uname=$(uname -sm)"

# --- A4/A6/A7: deterministic quality + hygiene gates ---------------------
run_step "A6 warning budget"        scripts/check_warning_budget.sh
run_step "A6 panic budget"          python3 scripts/check_panic_budget.py
run_step "A6 swallowed-error budget" python3 scripts/check_swallowed_error_budget.py
run_step "A6 advisory policy"       python3 scripts/check_advisory_policy.py
run_step "A4 code size budget"      python3 scripts/check_code_size_budget.py
run_step "A4 test size budget"      python3 scripts/check_test_size_budget.py
run_step "A4 wildcard reexport"     python3 scripts/check_wildcard_reexport_budget.py
run_step "A4 dependency boundaries" python3 scripts/check_dependency_boundaries.py
run_step "A4 tui render lock"       python3 scripts/check_tui_render_lock.py
run_step "A4 env lease drop order"  python3 scripts/check_env_lease_drop_order.py
run_step "A4 config env lease"      python3 scripts/check_config_env_lease.py
run_step "A6 agent instructions"    python3 scripts/check_agent_instructions.py
run_step "A6 docs references"       python3 scripts/check_docs_references.py
run_step "A7 ambient roots"         scripts/check_ambient_roots.sh
run_step "A7 real-home isolation"   scripts/check_real_home_isolation.sh

# --- A0-A3: real-process runtime lifecycle matrix ------------------------
# run_lifecycle_matrix.sh hardcodes its log to evidence/F14/, which S01 does
# not own. Back it up, run, fold the output into this round's transcript, then
# restore byte-identical and verify with diff -q. A harness that quietly
# rewrites another node's evidence is a scope violation, not a detail.
F14LOG="$REPO/docs/fork/ideal-base/evidence/F14/lifecycle_matrix_run.log"
# `mktemp -t PREFIX` is the BSD spelling. Under GNU coreutils (which is what
# the nix dev shell puts on PATH) the same argument is read as a TEMPLATE and
# rejected with "too few X's in template", leaving F14BAK empty. Round A hit
# exactly that: the backup was never taken, the restore silently no-oped, and
# F14's log was left holding S01's output. Use the portable XXXXXX form and
# make a failed backup fatal, so a missing backup can never again present as
# a completed restore.
F14BAK="$(mktemp "${TMPDIR:-/tmp}/s01f14.XXXXXX")" || {
    echo "FATAL: could not create F14 backup tempfile" >&2; exit 2; }
cp "$F14LOG" "$F14BAK" || {
    echo "FATAL: could not back up F14 evidence" >&2; exit 2; }
run_step "A0-A3 lifecycle matrix (1 round)" \
    bash scripts/run_lifecycle_matrix.sh 1
cat "$F14LOG" >> "$LOG"
cp "$F14BAK" "$F14LOG"
NSTEP=$((NSTEP + 1))
# Verify against F14's own pinned manifest, not just against the backup: a
# byte-identical copy of a wrong backup would still pass a bare diff.
if diff -q "$F14BAK" "$F14LOG" >/dev/null 2>&1 \
   && (cd "$REPO/docs/fork/ideal-base/evidence/F14" && shasum -a 256 -c SHA256SUMS >/dev/null 2>&1); then
    log "PASS F14 evidence restored byte-identical"
else
    log "FAIL F14 evidence NOT restored"
    FAIL=$((FAIL + 1))
fi
rm -f "$F14BAK"

# --- residue: nothing owned may survive the round ------------------------
orphans=$(pgrep -lf 'fake-mcp-server|crash-loop-mcp-server|hung-mcp-server|stale-gen-mcp|slow-mcp\.sh|owner-aware-mcp' 2>/dev/null || true)
NSTEP=$((NSTEP + 1))
if [[ -n "$orphans" ]]; then
    log "FAIL residue: orphaned fixture children: $orphans"
    FAIL=$((FAIL + 1))
else
    log "PASS residue: no orphaned fixture children"
fi

log "=== summary ==="
log "S01_ROUND=$LABEL N_STEP=$NSTEP N_FAIL=$FAIL"
exit "$FAIL"
