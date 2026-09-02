#!/usr/bin/env bash
# Remote test runner: cargo-nextest on a remote host via scripts/remote_build.sh.
#
# Campaign workers run tests many times per fix loop; running them on the local
# laptop competes with the active session for cores. This wrapper sends the
# nextest invocation to a dedicated test host (builds go to the build host,
# tests to the test host -- see ~/.config/jcode/remote-build.env).
#
# This is deliberately a THIN front-end. scripts/remote_build.sh already owns
# ssh preflight (exit 75 on unreachable host, no local fallback), worktree-keyed
# remote dirs, rsync with worktree-safe .git exclusion, the sync-fingerprint
# guard, and the `nix develop` fallback when cargo is not on the remote PATH.
# If you find yourself duplicating rsync/ssh logic here, stop and extend
# remote_build.sh instead.
#
# Note on incremental compilation: remote_build.sh forces CARGO_INCREMENTAL=0
# for test/check/clippy/bench/doc subcommands, but `nextest` is intentionally
# NOT in that list. Workers rerun near-identical targets in the same worktree
# many times, and incremental reruns are where wrapper latency pays or doesn't.
# Do not "fix" this by adding nextest to the incremental-off list.

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/test_remote.sh [--host H] [--local] [--warm] [nextest args...]

Runs `cargo nextest run --profile ci <args...>` on a remote host.
With no nextest args, defaults to `--workspace --lib --bins`.

Options:
  --host H    Remote SSH host (default: $JCODE_REMOTE_TEST_HOST, then
              $JCODE_REMOTE_HOST, from env or ~/.config/jcode/remote-build.env)
  --local     Run locally via scripts/cargo_exec.sh instead (loud escape hatch
              for when the remote box is down mid-campaign)
  --warm      Append --no-run: compile all test binaries, run nothing.
              Documented pre-warm step for fresh worktrees.
  -h, --help  Show this help

Workflow:
  Standard:                 scripts/test_remote.sh -p jcode-base
  Full default suite:       scripts/test_remote.sh
  Pre-warm a new worktree:  scripts/test_remote.sh --warm
      (cold-start compile tax is real: each fresh worktree pays a full
      test-profile compile on first contact; pre-warming overlaps it with
      worker ramp-up instead of blocking the first fix loop)
  Remote down mid-campaign: scripts/test_remote.sh --local -p jcode-base

Failure semantics:
  Remote unreachable => exit 75, no silent local fallback. Use --local
  explicitly if you need to run anyway.
EOF
}

log() {
    printf 'test_remote: %s\n' "$*" >&2
}

# Env scrub -- unconditional. These provider/hook overrides can leak from an
# agent session into a test child and change behavior under test. ssh does not
# propagate them anyway, so the scrub's real teeth are on the --local path;
# keeping it unconditional means callers can never get it wrong.
unset JCODE_FORCE_PROVIDER JCODE_ACTIVE_PROVIDER JCODE_RUNTIME_PROVIDER \
    JCODE_OPENROUTER_TRANSPORT_STATE JCODE_HOOKS_DISABLED JCODE_HOOK_TURN_END \
    JCODE_HOOK_TURN_START JCODE_HOOK_SESSION_START JCODE_HOOK_SESSION_END

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# shellcheck source=scripts/remote_config.sh
source "$repo_root/scripts/remote_config.sh"
jcode_load_remote_config

HOST_OVERRIDE=""
RUN_LOCAL=0
WARM=0
NEXTEST_ARGS=()

# nextest has no --host/--local/--warm flags, so recognizing ours anywhere in
# the arg list is safe; everything else passes through to nextest untouched.
while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)
            [[ $# -lt 2 ]] && { log "error: --host requires a value"; exit 2; }
            HOST_OVERRIDE="$2"
            shift 2
            ;;
        --local)
            RUN_LOCAL=1
            shift
            ;;
        --warm)
            WARM=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            NEXTEST_ARGS+=("$1")
            shift
            ;;
    esac
done

# Baked invocation: nextest run with the ci profile. The --workspace --lib
# --bins defaults apply ONLY when no nextest args are given, so
# `scripts/test_remote.sh -p jcode-base` narrows naturally.
argv=(nextest run --profile ci)
if [[ "${#NEXTEST_ARGS[@]}" -gt 0 ]]; then
    argv+=("${NEXTEST_ARGS[@]}")
else
    argv+=(--workspace --lib --bins)
fi
if [[ "$WARM" -eq 1 ]]; then
    argv+=(--no-run)
fi

output_file="$(mktemp)"
trap 'rm -f "$output_file"' EXIT

# Best-effort: parse nextest's final "Summary [ 198.234s] ..." line for the
# test-only clause of the timing summary. Never affects the exit code.
nextest_summary_seconds() {
    grep -Eo 'Summary \[ *[0-9]+(\.[0-9]+)?s' "$output_file" 2>/dev/null \
        | tail -n 1 \
        | grep -Eo '[0-9]+(\.[0-9]+)?' \
        || true
}

# Two campaign workers independently misread nextest's "filter matched zero
# tests" failure as an implementation failure. Label it before propagating.
explain_zero_match() {
    if grep -Eiq 'no tests to run|Starting 0 tests|[^0-9]0 tests run' "$output_file" 2>/dev/null; then
        log "filter matched no tests -- this is a filter/selection problem, not an implementation failure"
    fi
}

start_ts=$(date +%s)

if [[ "$RUN_LOCAL" -eq 1 ]]; then
    # Loud escape hatch, never silent: a worker lost a full suite run when the
    # remote lane died mid-run (ssh exit 255 after compile) and the result read
    # as a test failure. --local is the documented recovery route when the
    # remote box is down mid-campaign, and it is also the path where the env
    # scrub above actually bites (ssh never propagated those vars anyway).
    cat >&2 <<'EOF'
############################################################
# LOCAL FALLBACK - results may differ from CI
# (macOS local vs Linux remote/CI)
############################################################
EOF
    rc=0
    # JCODE_REMOTE_CARGO=0: cargo_exec.sh delegates to dev_cargo.sh, which
    # would otherwise route this right back to the remote build host.
    JCODE_REMOTE_CARGO=0 "$repo_root/scripts/cargo_exec.sh" "${argv[@]}" \
        2>&1 | tee "$output_file" || rc=${PIPESTATUS[0]}
    total=$(( $(date +%s) - start_ts ))
    tests_secs="$(nextest_summary_seconds)"
    if [[ -n "$tests_secs" ]]; then
        log "total ${total}s (local run; tests ${tests_secs}s per nextest summary)"
    else
        log "total ${total}s (local run)"
    fi
    if [[ "$rc" -ne 0 ]]; then
        explain_zero_match
    fi
    exit "$rc"
fi

# Host resolution: --host flag > JCODE_REMOTE_TEST_HOST > JCODE_REMOTE_HOST.
HOST="${HOST_OVERRIDE:-${JCODE_REMOTE_TEST_HOST:-${JCODE_REMOTE_HOST:-}}}"
if [[ -z "$HOST" ]]; then
    log "error: no remote test host configured"
    log "set JCODE_REMOTE_TEST_HOST (or JCODE_REMOTE_HOST) in the environment or"
    log "$(jcode_remote_config_path), or pass --host HOST"
    exit 2
fi

# Note: remote_build.sh scans positional args for --profile to guess the cargo
# build mode for artifact sync-back. It will see nextest's `--profile ci` and
# guess wrong, which is harmless because --no-sync-back disables that path.
#
# Remote unreachable => remote_build.sh's preflight exits 75 with no local
# fallback; PIPESTATUS keeps tee from masking it.
rc=0
"$repo_root/scripts/remote_build.sh" --host "$HOST" --no-sync-back "${argv[@]}" \
    2>&1 | tee "$output_file" || rc=${PIPESTATUS[0]}

total=$(( $(date +%s) - start_ts ))

# Phase split from remote_build.sh's machine-greppable phase lines. The
# sync-vs-execute split must stay visible: a regression in the sync path
# (e.g. rsync accidentally re-copying target/) has to show up immediately.
phase_seconds() {
    grep -Eo "remote_build: phase $1 [0-9]+s" "$output_file" 2>/dev/null \
        | tail -n 1 \
        | grep -Eo '[0-9]+s$' \
        | grep -Eo '[0-9]+' \
        || true
}
sync_secs="$(phase_seconds sync)"
remote_secs="$(phase_seconds remote)"
tests_secs="$(nextest_summary_seconds)"

summary="total ${total}s"
if [[ -n "$sync_secs" && -n "$remote_secs" ]]; then
    summary+=" (sync ${sync_secs}s, remote ${remote_secs}s"
    if [[ -n "$tests_secs" ]]; then
        summary+="; tests ${tests_secs}s per nextest summary"
    fi
    summary+=")"
elif [[ -n "$tests_secs" ]]; then
    summary+=" (tests ${tests_secs}s per nextest summary)"
fi
log "$summary"

if [[ "$rc" -ne 0 ]]; then
    explain_zero_match
fi
exit "$rc"
