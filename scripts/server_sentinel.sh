#!/usr/bin/env bash
# Independent liveness sentinel for the jcode daemon.
#
# Purpose: during selfdev work on the reload/shutdown seam, the daemon can die
# without exec'ing a successor (e.g. the 2026-07-19 reload -> accept-loop-failure
# race), stranding every attached client until a human respawns it. This script
# is the deliberately dumb safety net: it shares no code with the harness it
# guards, observes liveness with the *stable* binary, and after a generous
# grace period respawns the daemon using the approved shared-server channel.
#
# It never kills anything, never selects builds, and never touches sessions.
# Worst case it does nothing and you are exactly where you'd be without it.
#
# Usage:
#   scripts/server_sentinel.sh            # foreground loop (^C to stop)
#   scripts/server_sentinel.sh --once     # single probe, exit 0=alive 1=dead
#
# Log: ~/.jcode/sentinel.log (append-only observations; doubles as evidence).

set -euo pipefail

JCODE_HOME="${JCODE_HOME:-$HOME/.jcode}"
STABLE_BIN="$JCODE_HOME/builds/stable/jcode"
SHARED_BIN="$JCODE_HOME/builds/shared-server/jcode"
LOG="$JCODE_HOME/sentinel.log"
# Under launchd TMPDIR is unset; resolve the per-user temp dir the same way
# the daemon does (macOS confstr) before falling back to /tmp.
default_tmpdir() {
    if [[ -n "${TMPDIR:-}" ]]; then echo "$TMPDIR"; return; fi
    if command -v getconf >/dev/null 2>&1; then
        local d
        d="$(getconf DARWIN_USER_TEMP_DIR 2>/dev/null || true)"
        if [[ -n "$d" ]]; then echo "$d"; return; fi
    fi
    echo /tmp
}
SOCKET="${JCODE_SOCKET:-$(default_tmpdir | sed 's:/*$::')/jcode.sock}"
RELOAD_MARKER="$JCODE_HOME/reload-info"
SHUTDOWN_MARKER="$JCODE_HOME/state/shutdown-watchdog.json"

POLL_SECS="${SENTINEL_POLL_SECS:-10}"
# Consecutive failed probes before rescue. With POLL_SECS=10 this is a 30s
# grace window -- far above the ~2-7s a healthy reload handoff needs.
DEAD_THRESHOLD="${SENTINEL_DEAD_THRESHOLD:-3}"
# Never rescue more than once per window (seconds), so a crash-looping dev
# build cannot be resurrected in a tight loop.
RESCUE_COOLDOWN_SECS="${SENTINEL_RESCUE_COOLDOWN_SECS:-300}"

log() {
    printf '[%s] %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*" | tee -a "$LOG" >&2
}

probe() {
    # Liveness = the socket exists and something ACCEPTs on it. Use nc for a
    # pure connect test; no jcode code involved. (macOS nc does not support
    # -z with -U, so do a real connect with immediate EOF.)
    [[ -S "$SOCKET" ]] || return 1
    nc -U "$SOCKET" -w 2 </dev/null >/dev/null 2>&1
}

reload_in_flight() {
    # A fresh reload marker means a handoff is intentionally in progress.
    [[ -f "$RELOAD_MARKER" ]] || return 1
    local age
    age=$(( $(date +%s) - $(stat -f %m "$RELOAD_MARKER" 2>/dev/null || echo 0) ))
    (( age < 60 ))
}

intentional_shutdown() {
    # The daemon records its exit reason in a durable marker. A *fresh*
    # marker with an intentional reason (user stop / idle timeout) means the
    # daemon meant to die: stand down instead of resurrecting it. Failure
    # reasons (accept-loop-failure, reload-exec-failed) and stale/absent
    # markers (hard crash writes nothing) fall through to rescue.
    [[ -f "$SHUTDOWN_MARKER" ]] || return 1
    local age
    age=$(( $(date +%s) - $(stat -f %m "$SHUTDOWN_MARKER" 2>/dev/null || echo 0) ))
    (( age < 120 )) || return 1
    grep -qE '"reason":"(sigterm|persistent-idle|temporary-idle|temporary-owner-exit)"' \
        "$SHUTDOWN_MARKER" 2>/dev/null
}

rescue_binary() {
    # Prefer the approved shared-server channel (what a healthy reload would
    # have exec'd); fall back to stable. Never the raw dev build.
    if [[ -x "$SHARED_BIN" ]]; then echo "$SHARED_BIN"; else echo "$STABLE_BIN"; fi
}

rescue() {
    local bin
    bin="$(rescue_binary)"
    if [[ ! -x "$bin" ]]; then
        log "RESCUE_SKIPPED no executable rescue binary (checked $SHARED_BIN, $STABLE_BIN)"
        return 1
    fi
    log "RESCUE spawning daemon via $bin serve"
    # Detach fully; the daemon must not die with the sentinel.
    nohup "$bin" serve >>"$JCODE_HOME/sentinel-rescue.out" 2>&1 &
    disown || true
}

if [[ "${1:-}" == "--once" ]]; then
    if probe; then log "PROBE alive socket=$SOCKET"; exit 0
    else log "PROBE dead socket=$SOCKET"; exit 1; fi
fi

log "SENTINEL start socket=$SOCKET poll=${POLL_SECS}s threshold=$DEAD_THRESHOLD cooldown=${RESCUE_COOLDOWN_SECS}s"
consecutive_dead=0
last_rescue=0
while true; do
    if probe; then
        if (( consecutive_dead > 0 )); then
            log "RECOVERED after $consecutive_dead dead probe(s)"
        fi
        consecutive_dead=0
    else
        consecutive_dead=$((consecutive_dead + 1))
        if reload_in_flight; then
            log "DEAD probe $consecutive_dead/$DEAD_THRESHOLD (reload marker fresh; holding off)"
        elif intentional_shutdown; then
            log "DEAD probe $consecutive_dead/$DEAD_THRESHOLD (intentional shutdown marker; standing down)"
            consecutive_dead=0
        else
            log "DEAD probe $consecutive_dead/$DEAD_THRESHOLD socket=$SOCKET"
            if (( consecutive_dead >= DEAD_THRESHOLD )); then
                now=$(date +%s)
                if (( now - last_rescue >= RESCUE_COOLDOWN_SECS )); then
                    rescue && last_rescue=$now
                    consecutive_dead=0
                else
                    log "RESCUE_SUPPRESSED cooldown ($((now - last_rescue))s since last)"
                fi
            fi
        fi
    fi
    sleep "$POLL_SECS"
done
