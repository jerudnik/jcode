#!/usr/bin/env bash
# Verdict: WIRE
# Gate: `scripts/preflight.sh --ratchets-only`; the preflight owner maintains
# the invocation, and this script must stay green on the shared tree.
#
# Gate: no code outside `jcode-storage` may resolve an ambient filesystem root
# directly through `dirs::`.
#
# `crates/jcode-storage/src/lib.rs` is the single place that knows how to
# resolve the three ambient roots -- `jcode_dir()` (`~/.jcode`, honoring
# `JCODE_HOME`), `app_config_dir()` (the platform config dir), and
# `user_home_path()` (`~`) -- and the only place that redirects them to a
# per-process temp home under a test harness. A `dirs::home_dir()` call
# anywhere else silently opts that call site out of the redirect.
#
# This is a correctness gate, not a style gate. The bypass has already produced
# real defects: memory-log writes and the copilot `machine_id` landed in the
# developer's real `~/.jcode` even with `JCODE_HOME` redirected, and the model
# picker sorted routes by the developer's own usage history, which made five
# jcode-tui tests red locally and green in CI purely because CI's home is empty.
#
# How this differs from its two siblings, since all three sound alike:
#
#   - `check_real_home_isolation.sh` runs a suite and asserts no real root
#     gained files. It sees *writes* only; a read leak is invisible to it.
#   - `every_ambient_root_redirects_under_a_test_harness` (in jcode-storage)
#     asserts the helpers themselves redirect. It cannot see a caller that
#     never asks the helpers.
#   - this gate is the static half: it finds the callers that bypass the
#     helpers, which is the only way to catch a read leak before it ships.
#
# The allowlist is a ratchet. It names every remaining offender as `file:line`
# with a reason, and it may only shrink; an entry that no longer matches is
# itself an error, so the list cannot rot into a rubber stamp as code moves.
#
# Usage:
#   scripts/check_ambient_roots.sh            # fail on any non-allowlisted site
#   scripts/check_ambient_roots.sh --list     # print current sites, allowlist status
#   scripts/check_ambient_roots.sh --update   # rewrite the allowlist from reality
#                                             # (refuses to grow it)
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

allowlist_file="scripts/ambient_roots_allowlist.txt"

mode=check
for arg in "$@"; do
  case "$arg" in
    --list) mode=list ;;
    --update) mode=update ;;
    *) printf 'usage: %s [--list|--update]\n' "$0" >&2; exit 2 ;;
  esac
done

# The ambient-root accessors. `dirs::` is the crate every one of these came
# from; matching the call rather than the import keeps `use dirs;` and any
# aliasing from hiding a site.
# POSIX ERE for awk: no `\s`, and `(` must be escaped as `[(]` to stay literal.
pattern='dirs::(home_dir|config_dir|config_local_dir|data_dir|data_local_dir|cache_dir|executable_dir|runtime_dir|preference_dir|state_dir)[[:space:]]*[(]'

# Search every Rust source outside the one crate allowed to resolve roots.
#
# Line comments are stripped before matching. Without this the gate counted its
# own explanatory prose: a comment saying "use jcode_dir(), not dirs::home_dir()"
# registered as an offender, so documenting a fix made the count go up. That
# inflates the ratchet with entries no one can ever remove, which is precisely
# how an allowlist decays into noise. Only `//`-style comments are stripped,
# which covers `//`, `///` and `//!`; a `dirs::` call is not written inside a
# block comment anywhere in this tree, and stripping those properly needs a
# parser rather than a regex.
current_sites() {
  grep -rn --include='*.rs' -- '' crates src 2>/dev/null \
    | grep -v '^crates/jcode-storage/src/' \
    | awk -F: -v pat="$pattern" '
        {
          file = $1; line = $2
          # Reassemble the source text, which may itself contain colons.
          text = ""
          for (i = 3; i <= NF; i++) text = text (i > 3 ? ":" : "") $i
          sub(/\/\/.*/, "", text)
          if (text ~ pat) print file ":" line
        }' \
    | sort -u
}

# An allowlist entry is `path:line  # reason`. Comments and blanks are skipped.
allowlist_entries() {
  [ -f "$allowlist_file" ] || return 0
  sed 's/#.*//' "$allowlist_file" \
    | awk 'NF { gsub(/^[ \t]+|[ \t]+$/, ""); print }' \
    | sort -u
}

sites=$(current_sites || true)
allowed=$(allowlist_entries || true)

offenders=$(comm -23 <(printf '%s\n' "$sites" | grep -v '^$' || true) \
                     <(printf '%s\n' "$allowed" | grep -v '^$' || true) || true)
stale=$(comm -13 <(printf '%s\n' "$sites" | grep -v '^$' || true) \
                 <(printf '%s\n' "$allowed" | grep -v '^$' || true) || true)

site_count=$(printf '%s\n' "$sites" | grep -c . || true)
allow_count=$(printf '%s\n' "$allowed" | grep -c . || true)

case "$mode" in
  list)
    printf 'ambient roots: %s direct dirs:: site(s), %s allowlisted\n' \
      "$site_count" "$allow_count"
    printf '%s\n' "$sites" | grep . | while IFS= read -r site; do
      if printf '%s\n' "$allowed" | grep -qxF "$site"; then
        printf '  allowed   %s\n' "$site"
      else
        printf '  OFFENDER  %s\n' "$site"
      fi
    done
    exit 0
    ;;

  update)
    if [ "$site_count" -gt "$allow_count" ] && [ "$allow_count" -gt 0 ]; then
      printf 'refusing to grow the allowlist (%s sites > %s allowed).\n' \
        "$site_count" "$allow_count" >&2
      printf 'route the new call site through jcode-storage instead.\n' >&2
      exit 1
    fi
    tmp=$(mktemp)
    {
      printf '# Remaining direct `dirs::` ambient-root call sites (F29 ratchet).\n'
      printf '#\n'
      printf '# Regenerate with `scripts/check_ambient_roots.sh --update`, which\n'
      printf '# refuses to grow this list. Every entry needs a reason: the point is\n'
      printf '# that the remaining sites are deliberate, not merely tolerated.\n'
      printf '#\n'
      printf '# Format: path:line  # reason\n\n'
      printf '%s\n' "$sites" | grep . | while IFS= read -r site; do
        reason=$(printf '%s\n' "$allowed" | grep -xF "$site" >/dev/null 2>&1 \
          && grep -F "$site" "$allowlist_file" 2>/dev/null | head -1 | sed 's/^[^#]*//' \
          || true)
        if [ -n "$reason" ]; then
          printf '%s  %s\n' "$site" "$reason"
        else
          printf '%s  # TODO: state why this cannot use jcode-storage\n' "$site"
        fi
      done
    } > "$tmp"
    mv "$tmp" "$allowlist_file"
    printf 'wrote %s (%s site(s))\n' "$allowlist_file" "$site_count"
    exit 0
    ;;
esac

status=0

if [ -n "$(printf '%s' "$offenders" | tr -d '[:space:]')" ]; then
  printf 'FAIL: direct dirs:: ambient-root call site(s) outside jcode-storage:\n' >&2
  printf '%s\n' "$offenders" | grep . | while IFS= read -r site; do
    file=${site%:*}
    line=${site##*:}
    src=$(sed -n "${line}p" "$file" 2>/dev/null | sed 's/^[[:space:]]*//')
    printf '      %s\n          %s\n' "$site" "$src" >&2
  done
  printf '\n' >&2
  printf 'Resolve roots through jcode-storage instead:\n' >&2
  printf '  ~/.jcode (honors JCODE_HOME) -> jcode_storage::jcode_dir()\n' >&2
  printf '  platform config dir          -> jcode_storage::app_config_dir()\n' >&2
  printf '  user home (~ expansion)      -> jcode_storage::user_home_path()\n' >&2
  printf 'For a leaf crate without a storage dependency, prefer taking the path\n' >&2
  printf 'as a parameter over adding a dep edge.\n' >&2
  status=1
fi

# A stale entry means the code moved and the allowlist did not. Left unchecked,
# the list drifts until it no longer describes anything, which is how a ratchet
# quietly stops ratcheting.
if [ -n "$(printf '%s' "$stale" | tr -d '[:space:]')" ]; then
  printf 'FAIL: allowlist entr(ies) no longer match a real call site:\n' >&2
  printf '%s\n' "$stale" | grep . | sed 's/^/      /' >&2
  printf '  (the code moved; rerun with --update after confirming each site)\n' >&2
  status=1
fi

# An entry with a placeholder reason is worse than no entry: the gate's own
# success message claims every site is allowlisted "with a stated reason", so
# an unfilled `TODO` makes the gate assert something false. `--update` seeds
# entries with that TODO precisely so they are visible; failing here is what
# forces them to be filled in rather than accumulating.
unstated=$(grep -n 'TODO: state why' "$allowlist_file" 2>/dev/null || true)
if [ -n "$unstated" ]; then
  printf 'FAIL: allowlist entr(ies) still carry a placeholder reason:\n' >&2
  printf '%s\n' "$unstated" | sed 's/^/      /' >&2
  printf '  Each site needs a real reason for why it cannot use jcode-storage.\n' >&2
  status=1
fi

if [ "$status" -eq 0 ]; then
  if [ "$allow_count" -eq 0 ]; then
    printf 'ambient roots: ok (no direct dirs:: call sites outside jcode-storage)\n'
  else
    printf 'ambient roots: ok (%s site(s), all allowlisted with a stated reason)\n' \
      "$allow_count"
  fi
fi

exit "$status"
