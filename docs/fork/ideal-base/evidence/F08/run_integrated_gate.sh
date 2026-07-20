#!/usr/bin/env bash
# F08 integrated MCP + lifecycle adversarial gate.
#
# Repeatedly executes the accepted W1 matrices and checks for process/file
# residue after each round. Gates:
#   1. Zero surviving owned MCP descendants in every exit mode.
#   2. All activity/status durability matrices pass repeatedly.
#
# Usage: bash run_integrated_gate.sh [rounds]   (default 3)
# Writes: integrated_gate_run.log + residue_report.txt next to itself.

set -uo pipefail

ROUNDS="${1:-3}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../../../../.." && pwd)"
LOG="$HERE/integrated_gate_run.log"
RESIDUE="$HERE/residue_report.txt"
FAIL=0

# The F03 runtime matrix needs a jcode binary; prefer the current selfdev
# build, then the shared-server channel.
JCODE_BIN="${JCODE_BIN:-}"
if [[ -z "$JCODE_BIN" ]]; then
    for candidate in "$REPO/target/selfdev/jcode" "$HOME/.jcode/builds/shared-server/jcode"; do
        if [[ -x "$candidate" ]]; then JCODE_BIN="$candidate"; break; fi
    done
fi

# Background shells can inherit a stripped PATH (no nix, no cargo). Pin a
# PATH that includes every plausible toolchain location before recovery.
export PATH="$HOME/.cargo/bin:$HOME/.nix-profile/bin:/etc/profiles/per-user/$USER/bin:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:/usr/local/bin:/usr/bin:/bin:$PATH"

# scripts/dev_cargo.sh needs cargo; when run outside the dev shell (cron,
# background bash), recover the toolchain from rustup's default location.
if ! command -v cargo >/dev/null 2>&1 && [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi
# A background shell can inherit IN_NIX_SHELL with a stripped PATH, which
# makes dev_cargo refuse instead of re-entering the dev shell. Clear it so
# dev_cargo's own nix-reexec recovery path can run.
CARGO_RUNNER=(env -u IN_NIX_SHELL -u DEV_CARGO_NIX_REEXEC scripts/dev_cargo.sh)

log() { printf '[%s] %s\n' "$(date -u '+%H:%M:%S')" "$*" | tee -a "$LOG"; }

residue_check() {
    local phase="$1"
    {
        echo "== residue after $phase ($(date -u '+%Y-%m-%dT%H:%M:%SZ'))"
        # Orphaned fixture children from MCP tests (fake/crash-loop/stale-gen
        # servers). pgrep by script name; empty is a pass.
        local orphans
        orphans=$(pgrep -lf 'fake-mcp-server|crash-loop-mcp-server|stale-gen-mcp|slow-mcp\.sh|owner-aware-mcp' 2>/dev/null || true)
        if [[ -n "$orphans" ]]; then
            echo "FAIL orphaned MCP fixture children:"; echo "$orphans"
            FAIL=1
        else
            echo "PASS no orphaned MCP fixture children"
        fi
        # Stale test sockets left in temp dirs by shutdown fixtures.
        local socks
        socks=$(find "${TMPDIR:-/tmp}" -maxdepth 2 -name 'jcode-smoke*.sock' -mmin -30 2>/dev/null | head -5)
        if [[ -n "$socks" ]]; then
            echo "WARN recent smoke sockets (may belong to a live run):"; echo "$socks"
        else
            echo "PASS no fresh smoke-test sockets"
        fi
    } >> "$RESIDUE"
}

: > "$LOG"; : > "$RESIDUE"
log "F08 integrated gate: $ROUNDS round(s), repo=$REPO binary=$JCODE_BIN"
cd "$REPO"

for round in $(seq 1 "$ROUNDS"); do
    log "--- round $round/$ROUNDS ---"

    log "[1/4] F03 lease-class runtime matrix"
    if bash "$HERE/../F03/lease_class_fixtures.sh" "$JCODE_BIN" >> "$LOG" 2>&1; then
        log "PASS lease_class_fixtures"
    else
        log "FAIL lease_class_fixtures (exit $?)"; FAIL=1
    fi
    residue_check "round$round-lease-matrix"

    log "[2/4] shutdown coordinator suite (F03/R04)"
    if "${CARGO_RUNNER[@]}" test -p jcode-app-core --lib shutdown >> "$LOG" 2>&1; then
        log "PASS shutdown suite"
    else
        log "FAIL shutdown suite"; FAIL=1
    fi

    log "[3/4] MCP lifecycle suite (F06/F07: kill, hang, reconnect, cooldown)"
    if "${CARGO_RUNNER[@]}" test -p jcode-base --lib mcp >> "$LOG" 2>&1; then
        log "PASS mcp suite"
    else
        log "FAIL mcp suite"; FAIL=1
    fi
    residue_check "round$round-mcp-suite"

    log "[4/4] background status durability suite (F05)"
    if "${CARGO_RUNNER[@]}" test -p jcode-base --lib background >> "$LOG" 2>&1; then
        log "PASS background suite"
    else
        log "FAIL background suite"; FAIL=1
    fi
done

log "--- summary ---"
if [[ "$FAIL" -eq 0 ]]; then
    log "F08 INTEGRATED GATE: PASS ($ROUNDS rounds, all matrices, no residue)"
else
    log "F08 INTEGRATED GATE: FAIL (see $LOG and $RESIDUE)"
fi
exit "$FAIL"
