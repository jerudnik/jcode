#!/usr/bin/env bash
# Preflight: run the blocking CI gate set locally before pushing.
#
# Motivation: the fork's Quality Guardrails / fork-ci rails enforce several
# ratchets (swallowed-error, panic, code/test size, wildcard re-export, warning
# budget) plus rustfmt + a `-D warnings` clippy pass. Discovering a ratchet or
# clippy failure only after a ~30-minute CI round-trip is expensive, and some of
# these gates conflict in non-obvious ways (e.g. the swallowed-error ratchet
# forbids `.ok()` while clippy's manual_ok_err *wants* it). Running the whole
# set together, locally, catches those interactions in seconds-to-minutes.
#
# Speed choices:
#   * The python ratchets are pure text scans -- they run first and cost nothing.
#   * Rust checks use `cargo check` / `cargo clippy`, NOT `cargo build`: clippy
#     and check are codegen-free, so they are 2-4x faster than a full build and
#     are exactly what CI's blocking clippy/fmt steps run.
#   * `--nix` swaps the cargo clippy/check for crane derivations that reuse the
#     already-built ~900-crate dependency layer and the flake's pinned toolchain
#     (1.96.0), giving the exact answer CI's pinned rustup produces without a
#     cold cargo rebuild. Slower to start (Nix eval), but cache-backed.
#
# Usage:
#   scripts/preflight.sh                 # ratchets + cargo fmt/clippy (dev shell)
#   scripts/preflight.sh --nix           # ratchets + crane clippy (pinned toolchain)
#   scripts/preflight.sh --ratchets-only # just the fast python/text gates
#   scripts/preflight.sh --no-clippy     # skip the (slowest) clippy pass
#
# Exit non-zero if any gate fails; a summary lists every gate's status.
set -uo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root" || { printf 'preflight: cannot cd to repo root %s\n' "$repo_root" >&2; exit 2; }

for arg in "$@"; do
  case "$arg" in
    -h|--help)
      sed -n '2,28p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
  esac
done

# ── Toolchain guard ──────────────────────────────────────────────────────────
# Several gates need cargo on PATH (rustfmt, clippy, and check_dependency_
# boundaries.py which shells out to `cargo metadata`). Outside the Nix dev shell
# there is no cargo, so re-exec ourselves once under `nix develop` (guarded by a
# sentinel so a devshell that somehow lacks cargo cannot loop). Mirrors the
# recovery logic in scripts/dev_cargo.sh.
if ! command -v cargo >/dev/null 2>&1; then
  if [ -n "${IN_NIX_SHELL:-}" ]; then
    printf 'preflight: cargo missing even inside the Nix dev shell; check flake.nix.\n' >&2
    exit 127
  elif [ -z "${PREFLIGHT_NIX_REEXEC:-}" ] && [ -f "$repo_root/flake.nix" ]; then
    # Make sure `nix` itself is reachable (Determinate installs land here).
    for nixbin in /nix/var/nix/profiles/default/bin ~/.nix-profile/bin /run/current-system/sw/bin; do
      [ -x "$nixbin/nix" ] && export PATH="$nixbin:$PATH" && break
    done
    if command -v nix >/dev/null 2>&1; then
      printf 'preflight: cargo not on PATH; re-entering repo Nix dev shell...\n' >&2
      export PREFLIGHT_NIX_REEXEC=1
      exec nix develop "$repo_root" --command "$repo_root/scripts/preflight.sh" "$@"
    fi
    printf 'preflight: cargo not on PATH and nix not found; enter `nix develop` first.\n' >&2
    exit 127
  else
    printf 'preflight: cargo not found; enter `nix develop` or install rustup.\n' >&2
    exit 127
  fi
fi

# ── Options ──────────────────────────────────────────────────────────────────
use_nix=0
ratchets_only=0
run_clippy=1
for arg in "$@"; do
  case "$arg" in
    --nix) use_nix=1 ;;
    --ratchets-only) ratchets_only=1 ;;
    --no-clippy) run_clippy=0 ;;
    *) printf 'preflight: unknown option: %s\n' "$arg" >&2; exit 2 ;;
  esac
done

# ── Result tracking ──────────────────────────────────────────────────────────
declare -a names=()
declare -a states=()
overall=0

# run <label> <cmd...> : run a gate, record pass/fail, keep going on failure.
run() {
  local label="$1"; shift
  printf '\033[1;34m▸ %s\033[0m\n' "$label"
  if "$@"; then
    names+=("$label"); states+=("pass")
  else
    names+=("$label"); states+=("FAIL")
    overall=1
  fi
}

# ── 1. Fast text/ratchet gates (no compilation) ──────────────────────────────
run "swallowed-error ratchet"  python3 scripts/check_swallowed_error_budget.py
run "panic-prone ratchet"      python3 scripts/check_panic_budget.py
run "code-size ratchet"        python3 scripts/check_code_size_budget.py
run "test-size ratchet"        python3 scripts/check_test_size_budget.py
run "wildcard-reexport ratchet" python3 scripts/check_wildcard_reexport_budget.py
run "dependency boundaries"    python3 scripts/check_dependency_boundaries.py
run "config env lease"         python3 scripts/check_config_env_lease.py
run "agent instructions"       python3 scripts/check_agent_instructions.py
run "docs impact advisory"     python3 scripts/test_docs_impact_advisory.py
run "F20c removal clean"       bash -c 'scripts/f20c_removal_report.sh --stdout >/dev/null'
run "warning budget"           bash scripts/check_warning_budget.sh

# Unused-dependency gate, mirroring the "Enforce no unused dependencies" step in
# both ci.yml and fork-ci.yml. This was added after F20c: deleting the release
# acquisition subsystem orphaned jcode-app-core's flate2/sha2/tar, which every
# local gate passed over and CI caught only after the PR was opened. cargo
# machete is a manifest/source text scan, so it is fast and needs no build.
#
# cargo-machete is not in the repo devShell, so fetch it through `nix shell`
# when it is not already on PATH. Only a machine with neither skips, and the skip is reported in the
# summary so a missing tool can never be mistaken for a pass.
#
# The fallback was added after this gate's *second* escape: routing the ambient
# filesystem roots through jcode-storage orphaned jcode-setup-hints' `dirs`, and
# a local run that skipped the gate reported all-green while CI failed. A gate
# that silently downgrades to "skip" on the machine where it would have caught
# something is not a gate.
if command -v cargo-machete >/dev/null 2>&1; then
  run "unused dependencies"    cargo-machete
elif cargo machete --help >/dev/null 2>&1; then
  run "unused dependencies"    cargo machete
elif command -v nix >/dev/null 2>&1; then
  run "unused dependencies"    nix shell nixpkgs#cargo-machete --command cargo-machete
else
  names+=("unused dependencies"); states+=("skip")
  printf '\033[1;34m▸ unused dependencies\033[0m\n'
  printf 'cargo-machete and nix not found; CI still enforces this. Install with:\n'
  printf '  cargo install cargo-machete --locked\n'
fi

# Lint any fork-owned workflow files that changed (mirrors the nix.yml step).
if command -v actionlint >/dev/null 2>&1; then
  changed_wf=$(git diff --name-only --diff-filter=d HEAD -- '.github/workflows/*.yml' 2>/dev/null || true)
  if [ -n "$changed_wf" ]; then
    # shellcheck disable=SC2086
    run "actionlint (changed workflows)" actionlint $changed_wf
  fi
fi

if [ "$ratchets_only" -eq 1 ]; then
  printf '\n(--ratchets-only: skipping rustfmt/clippy)\n'
fi

# ── 2. Rust gates (codegen-free), scoped to THIS change ──────────────────────
# fork-ci's blocking fmt/clippy gates run the whole-tree check but only FAIL
# when a flagged file is fork-modified relative to the fork-point tag. Locally we
# scope tighter: files changed by THIS branch/worktree vs origin/main. Rationale:
#   * It catches anything the current change introduces (the thing a pre-push
#     check exists for) without drowning in pre-existing fork debt.
#   * It avoids false stops on platform-gated lints CI cannot see. Example: a
#     `#[cfg(target_os = "macos")]` fn can carry a clippy lint that Linux CI
#     never compiles, so blocking on it locally would diverge from CI. Those
#     live in files this change did not touch, so scoping to the branch diff
#     drops them. (Pre-existing debt is the warning-budget ratchet's job.)
# Set PREFLIGHT_BASE to override the comparison base (default origin/main).
fork_touched_file=""
compute_fork_touched() {
  fork_touched_file=$(mktemp)
  local base="${PREFLIGHT_BASE:-origin/main}"
  if ! git rev-parse --verify -q "$base" >/dev/null; then
    # Fall back to the local main, then to an empty set (flag nothing extra).
    if git rev-parse --verify -q main >/dev/null; then base="main"; else base=""; fi
  fi
  if [ -n "$base" ]; then
    # Committed changes on this branch vs base...
    git diff --name-only --diff-filter=d "$base"...HEAD -- '*.rs' 2>/dev/null \
      > "$fork_touched_file"
    printf '  (clippy/fmt scoped to files changed vs %s + working tree)\n' "$base"
  fi
  # ...plus uncommitted working-tree changes, so a pre-commit run covers exactly
  # what you are about to push.
  git diff --name-only --diff-filter=d -- '*.rs' 2>/dev/null >> "$fork_touched_file"
  sort -u -o "$fork_touched_file" "$fork_touched_file"
}

# fmt/clippy, blocking only for fork-touched files. $1 = label, rest = the
# clippy/fmt cargo args that emit `--message-format=json` (clippy) or a
# check-style diff (fmt). We special-case the two because their output shapes
# differ; keeping them here avoids a second whole-tree compile.
if [ "$ratchets_only" -eq 0 ]; then
  compute_fork_touched

  # rustfmt: flag files with diffs, block only fork-touched ones.
  # shellcheck disable=SC2329  # invoked indirectly via run
  fmt_scoped() {
    local out flagged
    out=$(cargo fmt --all -- --check 2>&1) || true
    flagged=$(printf '%s\n' "$out" | grep '^Diff in ' \
      | sed "s|^Diff in $PWD/||; s|:[0-9]*:\$||" | sort -u)
    local bad
    bad=$(comm -12 "$fork_touched_file" <(printf '%s\n' "$flagged") 2>/dev/null)
    if [ -n "$bad" ]; then
      printf '%s\n' "$out"
      printf 'rustfmt diffs in fork-modified files:\n%s\n' "$bad"
      return 1
    fi
    return 0
  }
  run "rustfmt --check (fork-touched)" fmt_scoped

  if [ "$run_clippy" -eq 1 ]; then
    if [ "$use_nix" -eq 1 ]; then
      # Crane clippy: reuses the cached dependency layer + pinned toolchain.
      export PATH="/nix/var/nix/profiles/default/bin:${PATH}"
      run "crane clippy (pinned 1.96.0)" \
        nix build '.#clippy' --no-link --print-build-logs --accept-flake-config
    else
      # cargo clippy over the whole tree (no -D warnings so it completes), then
      # fail only when a lint's primary span lands in a fork-touched file.
      # Codegen-free, so far cheaper than a full build.
      # shellcheck disable=SC2329  # invoked indirectly via run
      clippy_scoped() {
        local json flagged bad
        json=$(mktemp)
        cargo clippy --all-targets --all-features --message-format=json \
          > "$json" 2>/dev/null || true
        flagged=$(jq -r '
          select(.reason=="compiler-message")
          | .message | select(.level=="warning" or .level=="error")
          | .spans[]? | select(.is_primary) | .file_name' "$json" \
          | grep -v '^/' | sort -u)
        bad=$(comm -12 "$fork_touched_file" <(printf '%s\n' "$flagged") 2>/dev/null)
        if [ -n "$bad" ]; then
          printf 'clippy lints in fork-modified files:\n%s\n\n' "$bad"
          # Render every clippy warning/error so the offending lines are visible;
          # the `bad` list above says which files actually block the push.
          jq -r '
            select(.reason=="compiler-message")
            | .message | select(.level=="warning" or .level=="error")
            | .rendered' "$json" 2>/dev/null || true
          rm -f "$json"
          return 1
        fi
        rm -f "$json"
        return 0
      }
      run "cargo clippy (fork-touched)" clippy_scoped
    fi
  fi

  # Real-home isolation. Runs an actual suite (~2 min), so it is opt-in locally
  # via PREFLIGHT_HOME_ISOLATION=1; CI asserts the same invariant for free with
  # --verify-only after its own test steps. Placed here rather than with the
  # fast ratchets because it compiles and runs tests.
  if [ "${PREFLIGHT_HOME_ISOLATION:-0}" = "1" ]; then
    run "real-home isolation" bash scripts/check_real_home_isolation.sh
  fi
fi

# ── Summary ──────────────────────────────────────────────────────────────────
printf '\n\033[1m── preflight summary ──\033[0m\n'
skipped=0
for i in "${!names[@]}"; do
  case "${states[$i]}" in
    pass) printf '  \033[32m✓\033[0m %s\n' "${names[$i]}" ;;
    # A skipped gate is neither a pass nor a failure: the check did not run
    # here, but CI still enforces it. Rendering it as ✗ would contradict the
    # "all gates passed" footer; rendering it as ✓ would hide the hole.
    skip) printf '  \033[33m—\033[0m %s \033[33m(skipped, enforced in CI)\033[0m\n' "${names[$i]}"
          skipped=$((skipped + 1)) ;;
    *)    printf '  \033[31m✗ %s\033[0m\n' "${names[$i]}" ;;
  esac
done

if [ "$overall" -eq 0 ]; then
  if [ "$skipped" -gt 0 ]; then
    printf '\n\033[1;32mAll preflight gates passed\033[0m \033[33m(%d skipped; CI still enforces them).\033[0m\n' "$skipped"
  else
    printf '\n\033[1;32mAll preflight gates passed.\033[0m\n'
  fi
else
  printf '\n\033[1;31mPreflight found failures above. Fix before pushing.\033[0m\n'
fi
exit "$overall"
