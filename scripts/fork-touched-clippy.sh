#!/usr/bin/env bash
# Local port of the CI "blocking only for fork-touched files" quality gate.
#
# Mirrors the `quality` job's "Clippy (blocking for fork-touched files)" and
# "Check formatting (blocking for fork-touched files)" steps that historically
# lived in the fork CI workflow so you can reproduce the CI verdict before (or
# after) a push. The logic is byte-parallel to CI: run the tool over the whole
# tree without -D warnings, then fail only when a lint's / rustfmt's primary
# span lands in a file this fork modified relative to the fork point. Drift in
# untouched upstream files is reported as a warning and tolerated, like CI.
#
# The fork-touched set is:
#   git diff --name-only --diff-filter=d <base-ref> HEAD -- '*.rs' | sort -u
# where <base-ref> is the `fork-point` tag: the upstream commit this fork
# diverged from. It was the `vendor/upstream` branch tip until the hard fork
# retired that rail; since the fork no longer follows upstream, the base is a
# fixed historical commit and a tag says so more honestly than a branch nobody
# advances. Override with --base-ref (--vendor-ref remains as an alias).
#
# cargo is invoked through scripts/dev_cargo.sh when present (Nix-aware); if
# cargo is not on PATH but a flake.nix + nix exist, the whole cargo invocation
# runs under `nix develop --command` so the repo toolchain is available.
#
# Usage:
#   scripts/fork-touched-clippy.sh [--fmt] [--clippy] [--base-ref <ref>]
#
# Modes (choose any combination; default is clippy only):
#   (no flag)          run the clippy gate only
#   --clippy           run the clippy gate
#   --fmt              run the rustfmt gate
#   --fmt --clippy     run both gates
#
# Options:
#   --base-ref <ref>   override the fork-point base ref used for the touched set
#   --vendor-ref <ref> deprecated alias for --base-ref
#   -h, --help         show this help and exit
#
# Exit codes:
#   0  no fork-touched lint/format issues (untouched-upstream drift is a warning)
#   1  a lint / rustfmt diff lands in a fork-modified file
#   2  usage error or the base ref could not be resolved
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$repo_root" ]]; then
  printf 'error: not inside a git repository\n' >&2
  exit 2
fi
cd "$repo_root"

vendor_ref_override=""
do_fmt="false"
do_clippy_explicit="false"

usage() {
  sed -n '2,37p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fmt) do_fmt="true" ;;
    --clippy) do_clippy_explicit="true" ;;
    --base-ref|--vendor-ref)
      [[ $# -ge 2 ]] || { printf 'error: %s requires an argument\n' "$1" >&2; exit 2; }
      vendor_ref_override="$2"; shift ;;
    --base-ref=*) vendor_ref_override="${1#--base-ref=}" ;;
    --vendor-ref=*) vendor_ref_override="${1#--vendor-ref=}" ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'error: unknown option: %s\n' "$1" >&2; exit 2 ;;
  esac
  shift
done

# Default (no gate flags) runs clippy only; --fmt alone runs fmt only; passing
# both flags runs both gates.
do_clippy="true"
if [[ "$do_fmt" == "true" && "$do_clippy_explicit" == "false" ]]; then
  do_clippy="false"
fi

fail() { printf 'FAIL: %s\n' "$*" >&2; }
ok()   { printf 'OK:   %s\n' "$*"; }
warn() { printf 'WARN: %s\n' "$*"; }

# ── Resolve the cargo runner and whether it must run under Nix ────────────────
if [[ -f scripts/dev_cargo.sh ]]; then
  cargo_cmd=("$repo_root/scripts/dev_cargo.sh")
else
  cargo_cmd=(cargo)
fi

use_nix="false"
if ! command -v cargo >/dev/null 2>&1 \
  && [[ -f flake.nix ]] \
  && command -v nix >/dev/null 2>&1; then
  use_nix="true"
fi

# Run the chosen cargo, capturing stdout (and stderr when $2 is true) into $1.
# Returns cargo's exit status. When wrapped in `nix develop`, the redirect is
# performed *inside* the inner shell so the devshell shellHook banner (printed
# to the outer stdout on entry) never contaminates the captured output.
cargo_to_file() {
  local outfile="$1" merge_stderr="$2"; shift 2
  if [[ "$use_nix" == "true" ]]; then
    if [[ "$merge_stderr" == "true" ]]; then
      # shellcheck disable=SC2016  # $0/$@ expand in the inner shell, by design.
      nix develop --command sh -lc '"$@" > "$0" 2>&1' \
        "$outfile" "${cargo_cmd[@]}" "$@"
    else
      # shellcheck disable=SC2016  # $0/$@ expand in the inner shell, by design.
      nix develop --command sh -lc '"$@" > "$0"' \
        "$outfile" "${cargo_cmd[@]}" "$@"
    fi
  else
    if [[ "$merge_stderr" == "true" ]]; then
      "${cargo_cmd[@]}" "$@" > "$outfile" 2>&1
    else
      "${cargo_cmd[@]}" "$@" > "$outfile"
    fi
  fi
}

# ── Resolve the vendor base ref ───────────────────────────────────────────────
# Resolve the base commit the fork-touched set is computed against.
resolve_vendor_ref() {
  local ref
  if [[ -n "$vendor_ref_override" ]]; then
    if git rev-parse --verify --quiet "${vendor_ref_override}^{commit}" >/dev/null; then
      printf '%s\n' "$vendor_ref_override"
      return 0
    fi
    printf 'error: base ref %s does not resolve to a commit\n' "$vendor_ref_override" >&2
    return 1
  fi
  # `fork-point` first: it is the post-hard-fork anchor and is immutable. The
  # vendor branches are tried only as a fallback for clones fetched before the
  # rail was retired.
  for ref in fork-point github/vendor/upstream origin/vendor/upstream; do
    if git rev-parse --verify --quiet "${ref}^{commit}" >/dev/null; then
      printf '%s\n' "$ref"
      return 0
    fi
  done
  cat >&2 <<'EOF'
error: no fork-point ref found (tried fork-point, github/vendor/upstream, origin/vendor/upstream).
The `fork-point` tag anchors the fork-touched file set. Fetch tags:
    git fetch --tags github
Then re-run, or pass --base-ref <ref> explicitly.
EOF
  return 1
}

vendor_ref="$(resolve_vendor_ref)" || exit 2

echo "=== fork-touched gate (base ref: $vendor_ref) ==="

# Temp workspace, cleaned up on exit.
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/fork-touched-clippy.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT

touched_file="$work_dir/fork-touched"
git diff --name-only --diff-filter=d "$vendor_ref" HEAD -- '*.rs' \
  | sort -u > "$touched_file"
touched_count="$(wc -l < "$touched_file" | tr -d ' ')"
ok "fork-touched .rs files vs $vendor_ref: $touched_count"

exit_code=0

# ── Clippy gate ───────────────────────────────────────────────────────────────
run_clippy_gate() {
  echo "--- clippy (whole-tree, blocking for fork-touched files) ---"
  local clippy_json="$work_dir/clippy.json"
  local flagged="$work_dir/clippy-flagged"
  local clippy_status=0
  cargo_to_file "$clippy_json" "false" \
    clippy --locked --workspace --all-targets --all-features --message-format=json \
    || clippy_status=$?
  if [[ "$clippy_status" -ne 0 ]]; then
    warn "cargo clippy exited $clippy_status (possible hard compile error; run cargo check)"
  fi

  jq -r '
    select(.reason=="compiler-message")
    | .message
    | select(.level=="warning" or .level=="error")
    | .spans[]?
    | select(.is_primary)
    | .file_name' "$clippy_json" \
    | grep -v '^/' | sort -u > "$flagged" || true

  local bad vendor_only
  bad="$(comm -12 "$touched_file" "$flagged")"
  if [[ -n "$bad" ]]; then
    fail "clippy lints in fork-modified files:"
    printf '%s\n' "$bad" >&2
    jq -r '
      select(.reason=="compiler-message")
      | .message
      | select(.level=="warning" or .level=="error")
      | .rendered' "$clippy_json"
    return 1
  fi
  vendor_only="$(comm -13 "$touched_file" "$flagged")"
  if [[ -n "$vendor_only" ]]; then
    warn "untouched upstream files carry clippy lints (not blocking):"
    printf '%s\n' "$vendor_only"
  else
    ok "clippy clean across the whole tree."
  fi
  return 0
}

# ── Formatting gate ───────────────────────────────────────────────────────────
run_fmt_gate() {
  echo "--- rustfmt (whole-tree, blocking for fork-touched files) ---"
  local fmt_out="$work_dir/fmt-out"
  local flagged="$work_dir/fmt-flagged"
  cargo_to_file "$fmt_out" "true" fmt --all -- --check || true
  grep '^Diff in ' "$fmt_out" \
    | sed "s|^Diff in $PWD/||; s|:[0-9]*:\$||" \
    | sort -u > "$flagged" || true

  local bad vendor_only
  bad="$(comm -12 "$touched_file" "$flagged")"
  if [[ -n "$bad" ]]; then
    fail "rustfmt diffs in fork-modified files:"
    printf '%s\n' "$bad" >&2
    cat "$fmt_out"
    return 1
  fi
  vendor_only="$(comm -13 "$touched_file" "$flagged")"
  if [[ -n "$vendor_only" ]]; then
    warn "untouched upstream files fail rustfmt (not blocking):"
    printf '%s\n' "$vendor_only"
  else
    ok "cargo fmt clean across the whole tree."
  fi
  return 0
}

if [[ "$do_clippy" == "true" ]]; then
  run_clippy_gate || exit_code=1
fi
if [[ "$do_fmt" == "true" ]]; then
  run_fmt_gate || exit_code=1
fi

echo
if [[ "$exit_code" -eq 0 ]]; then
  echo "=== fork-touched gate: clean ==="
else
  echo "=== fork-touched gate: blocking issue(s) in fork-modified files ===" >&2
fi
exit "$exit_code"
