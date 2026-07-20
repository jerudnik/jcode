#!/usr/bin/env bash
# F14: complete no-provider real-process lifecycle matrix.
#
# Composes every accepted W1/W2 runtime invariant into one rerunnable
# harness. Gates:
#   1. Repeated clean runs cover cancel, exit, reload, crash, restart,
#      recovery, and residue.
#   2. No provider/network dependency (env strips provider keys; fixtures
#      use real daemons + fake MCP servers only).
#
# Usage: scripts/run_lifecycle_matrix.sh [rounds]   (default 2)
# Log: docs/fork/ideal-base/evidence/F14/lifecycle_matrix_run.log

set -uo pipefail

ROUNDS="${1:-2}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVID="$REPO/docs/fork/ideal-base/evidence/F14"
mkdir -p "$EVID"
LOG="$EVID/lifecycle_matrix_run.log"
FAIL=0

# Toolchain recovery for stripped shells (same approach as the F08 gate).
export PATH="$HOME/.cargo/bin:$HOME/.nix-profile/bin:/etc/profiles/per-user/$USER/bin:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
CARGO=(env -u IN_NIX_SHELL -u DEV_CARGO_NIX_REEXEC "$REPO/scripts/dev_cargo.sh")

# Gate 2: no provider/network dependency. Strip every provider credential
# so any accidental provider call fails loudly instead of silently working.
export ANTHROPIC_API_KEY="" OPENAI_API_KEY="" GEMINI_API_KEY="" \
       OPENROUTER_API_KEY="" GROQ_API_KEY="" JCODE_NO_TELEMETRY=1

JCODE_BIN="${JCODE_BIN:-}"
if [[ -z "$JCODE_BIN" ]]; then
    for candidate in "$REPO/target/selfdev/jcode" "$HOME/.jcode/builds/shared-server/jcode"; do
        [[ -x "$candidate" ]] && { JCODE_BIN="$candidate"; break; }
    done
fi

log() { printf '[%s] %s\n' "$(date -u '+%H:%M:%S')" "$*" | tee -a "$LOG"; }
run_step() {
    local label="$1"; shift
    if "$@" >> "$LOG" 2>&1; then
        log "PASS $label"
    else
        log "FAIL $label (exit $?)"; FAIL=1
    fi
}

: > "$LOG"
log "F14 lifecycle matrix: rounds=$ROUNDS binary=$JCODE_BIN (providers stripped)"
cd "$REPO"

for round in $(seq 1 "$ROUNDS"); do
    log "=== round $round/$ROUNDS ==="

    # -- exit / crash / restart / recovery / residue (real processes) -----
    # F03 matrix: per-lease-class idle behavior, SIGTERM (exit 0 drain),
    # SIGKILL (crash) + successor boot over stale socket, residue checks.
    run_step "lease/exit/crash/restart matrix (F03)" \
        bash docs/fork/ideal-base/evidence/F03/lease_class_fixtures.sh "$JCODE_BIN"

    # -- reload (in-process state machine incl. R04 classification) ------
    run_step "shutdown+reload coordinator suite (F02/F03/R04)" \
        "${CARGO[@]}" test -p jcode-app-core --lib shutdown
    run_step "reload state/recovery suites (R01)" \
        "${CARGO[@]}" test -p jcode-app-core --lib reload

    # -- cancel ----------------------------------------------------------
    # Turn/stream cancellation invariants (R12 fixtures) + interrupt paths.
    run_step "turn cancel/interrupt suite" \
        "${CARGO[@]}" test -p jcode-app-core --lib agent::

    # -- MCP child lifecycle: kill, hang, reconnect, cooldown, caps ------
    run_step "mcp lifecycle suite (F06/F07/F12)" \
        "${CARGO[@]}" test -p jcode-base --lib mcp

    # -- background durability, orphan recovery, caps --------------------
    run_step "background durability+orphan suite (F04/F05/F12)" \
        "${CARGO[@]}" test -p jcode-base --lib background

    # -- disconnect cleanup + startup reconciliation (F09/F10) -----------
    run_step "disconnect-cleanup suite (F10)" \
        "${CARGO[@]}" test -p jcode-app-core --lib client_disconnect_cleanup
    run_step "pending-activation reconcile suite (F09)" \
        "${CARGO[@]}" test -p jcode-build-support reconcile

    # -- residue ---------------------------------------------------------
    orphans=$(pgrep -lf 'fake-mcp-server|crash-loop-mcp-server|hung-mcp-server|stale-gen-mcp|slow-mcp\.sh|owner-aware-mcp' 2>/dev/null || true)
    if [[ -n "$orphans" ]]; then
        log "FAIL residue: orphaned MCP fixture children: $orphans"; FAIL=1
    else
        log "PASS residue: no orphaned fixture children"
    fi
done

log "=== summary ==="
if [[ "$FAIL" -eq 0 ]]; then
    log "F14 LIFECYCLE MATRIX: PASS ($ROUNDS rounds)"
else
    log "F14 LIFECYCLE MATRIX: FAIL"
fi
exit "$FAIL"
