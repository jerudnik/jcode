#!/usr/bin/env bash
# Emergency recovery for a jcode daemon that cannot be restarted normally.
#
# Escalation ladder (each stage only runs if the previous one cannot help):
#   1. rollback  -- smoke-test known-good binaries (stable channel, nix store),
#                   repoint the shared-server channel at the first working one,
#                   and spawn it. Recovers from "the promoted build is broken".
#   2. summon    -- launch an external agent (claude CLI) with a repair brief.
#                   Recovers from environmental breakage a script can't reason
#                   about. Heavily rate-limited; writes a brief to disk so the
#                   summoned agent has context.
#
# Invoked automatically by server_sentinel.sh after repeated failed rescues,
# or manually:
#   scripts/jcode_emergency.sh rollback
#   scripts/jcode_emergency.sh summon
#   scripts/jcode_emergency.sh auto      # rollback, then summon if still dead
#
# Design constraints: no jcode library code, no build system, bash + coreutils
# only. Every action is logged to ~/.jcode/emergency.log.

set -euo pipefail

JCODE_HOME="${JCODE_HOME:-$HOME/.jcode}"
LOG="$JCODE_HOME/emergency.log"
BUILDS="$JCODE_HOME/builds"
SUMMON_STAMP="$JCODE_HOME/state/emergency-summon-stamp"
SUMMON_COOLDOWN_SECS="${EMERGENCY_SUMMON_COOLDOWN_SECS:-3600}"

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

log() {
    printf '[%s] %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*" | tee -a "$LOG" >&2
}

daemon_alive() {
    [[ -S "$SOCKET" ]] && nc -U "$SOCKET" -w 2 </dev/null >/dev/null 2>&1
}

# A binary is "known good" if it can print its version within a few seconds.
smoke() {
    local bin="$1"
    [[ -x "$bin" ]] || return 1
    timeout 10 "$bin" version >/dev/null 2>&1 || timeout 10 "$bin" --version >/dev/null 2>&1
}

nix_binary() {
    # The nix-managed jcode is built by an entirely separate pipeline; find it
    # via the user profile first, then any store path.
    local candidates=(
        "/etc/profiles/per-user/$USER/bin/jcode"
        "$HOME/.nix-profile/bin/jcode"
    )
    for c in "${candidates[@]}"; do
        if [[ -x "$c" ]]; then echo "$c"; return 0; fi
    done
    ls -d /nix/store/*-jcode-nix-managed/bin/jcode 2>/dev/null | head -1
}

rollback() {
    log "ROLLBACK start (socket=$SOCKET)"
    if daemon_alive; then
        log "ROLLBACK skipped: daemon is alive"
        return 0
    fi

    local candidates=()
    [[ -x "$BUILDS/stable/jcode" ]] && candidates+=("$BUILDS/stable/jcode")
    local nix_bin
    nix_bin="$(nix_binary || true)"
    [[ -n "$nix_bin" ]] && candidates+=("$nix_bin")

    local bin
    for bin in "${candidates[@]}"; do
        if ! smoke "$bin"; then
            log "ROLLBACK candidate failed smoke test: $bin"
            continue
        fi
        log "ROLLBACK candidate passed smoke test: $bin"
        # Repoint shared-server at the known-good payload so future reloads
        # and the sentinel's normal rescue path also use it. Resolve symlinks
        # to an immutable payload when possible.
        local payload
        payload="$(readlink -f "$bin" 2>/dev/null || echo "$bin")"
        if [[ -d "$BUILDS/shared-server" ]]; then
            ln -sfn "$payload" "$BUILDS/shared-server/jcode"
            log "ROLLBACK repointed shared-server -> $payload"
        fi
        log "ROLLBACK spawning daemon via $bin serve"
        nohup "$bin" serve >>"$JCODE_HOME/emergency-rescue.out" 2>&1 &
        disown || true
        # Give it a moment, then verify.
        local i
        for i in 1 2 3 4 5 6; do
            sleep 5
            if daemon_alive; then
                log "ROLLBACK SUCCESS: daemon alive on $bin"
                return 0
            fi
        done
        log "ROLLBACK spawn did not become live within 30s: $bin"
    done
    log "ROLLBACK FAILED: no candidate produced a live daemon"
    return 1
}

summon() {
    log "SUMMON requested"
    if daemon_alive; then
        log "SUMMON skipped: daemon is alive"
        return 0
    fi
    # Rate limit: summoning spends external-agent quota and should never loop.
    if [[ -f "$SUMMON_STAMP" ]]; then
        local age
        age=$(( $(date +%s) - $(stat -f %m "$SUMMON_STAMP" 2>/dev/null || echo 0) ))
        if (( age < SUMMON_COOLDOWN_SECS )); then
            log "SUMMON suppressed: cooldown (${age}s since last, limit ${SUMMON_COOLDOWN_SECS}s)"
            return 1
        fi
    fi
    local agent=""
    if command -v claude >/dev/null 2>&1; then agent="claude"
    elif command -v codex >/dev/null 2>&1; then agent="codex"
    fi
    if [[ -z "$agent" ]]; then
        log "SUMMON FAILED: no external agent CLI found (tried claude, codex)"
        return 1
    fi

    mkdir -p "$JCODE_HOME/state"
    local brief="$JCODE_HOME/state/emergency-brief.md"
    {
        echo "# jcode emergency repair brief"
        echo
        echo "Generated: $(date -u '+%Y-%m-%dT%H:%M:%SZ') by jcode_emergency.sh"
        echo
        echo "The jcode daemon cannot be started. Automatic rollback to the"
        echo "stable and nix binaries has already FAILED. Your job: diagnose"
        echo "why no daemon will listen on $SOCKET and repair it."
        echo
        echo "Facts:"
        echo "- Socket: $SOCKET (exists: $([[ -e "$SOCKET" ]] && echo yes || echo no))"
        echo "- Repo: $HOME/labs/jcode"
        echo "- Emergency log: $LOG (read the tail first)"
        echo "- Sentinel log: $JCODE_HOME/sentinel.log"
        echo "- Daemon exit marker: $JCODE_HOME/state/shutdown-watchdog.json"
        echo "- Recent daemon logs: $JCODE_HOME/logs/ (newest jcode-*.log)"
        echo "- Rescue spawn output: $JCODE_HOME/emergency-rescue.out"
        echo "- Builds: $BUILDS (channels: stable, shared-server, current)"
        echo
        echo "Suggested checks: stale socket file, port/file locks, corrupt"
        echo "config ($JCODE_HOME/config*.toml), disk full, binary smoke"
        echo "failures in the emergency log. Prefer minimal repairs; do not"
        echo "rebuild from source unless every binary is broken."
        echo
        echo "Success = 'nc -U $SOCKET </dev/null' connects."
    } > "$brief"

    touch "$SUMMON_STAMP"
    log "SUMMON launching $agent with brief $brief"
    case "$agent" in
        claude)
            nohup claude -p "Read $brief and repair the jcode daemon as instructed. Work autonomously." \
                --dangerously-skip-permissions \
                >>"$JCODE_HOME/emergency-summon.out" 2>&1 &
            ;;
        codex)
            nohup codex exec --full-auto "Read $brief and repair the jcode daemon as instructed. Work autonomously." \
                >>"$JCODE_HOME/emergency-summon.out" 2>&1 &
            ;;
    esac
    disown || true
    log "SUMMON launched $agent (output: $JCODE_HOME/emergency-summon.out)"
}

case "${1:-auto}" in
    rollback) rollback ;;
    summon)   summon ;;
    auto)
        if rollback; then exit 0; fi
        summon
        ;;
    *)
        echo "usage: $0 [rollback|summon|auto]" >&2
        exit 2
        ;;
esac
