#!/usr/bin/env bash
# Build the current source and publish it to jcode's single fixed binary path,
# then point the launcher at it.
#
# F20b made ~/.jcode/current/jcode the ONE path every jcode client and daemon
# resolves to, and F20c deleted the version store plus the
# stable/current/shared-server/canary channel symlinks this script used to
# write. Writing those channels now would produce state that nothing reads, so
# this publishes to the fixed path instead.
#
# Paths after install:
# - ~/.jcode/current/jcode          (the single fixed publish target)
# - ~/.local/bin/jcode -> ~/.jcode/current/jcode (launcher)
#
# The publish is a staged copy + atomic rename, matching the Rust publish path
# (crates/jcode-build-support: publish_current_fixed), so a running daemon can
# never exec a half-written binary.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

profile="${JCODE_RELEASE_PROFILE:-release-lto}"
if [[ "${1:-}" == "--fast" ]]; then
  profile="release"
  shift
fi

if [[ "$#" -gt 0 ]]; then
  echo "Usage: $0 [--fast]" >&2
  exit 1
fi

case "$profile" in
  release-lto)
    echo "Building with LTO (this takes a few minutes)..."
    ;;
  release)
    echo "Building fast release profile (no LTO)..."
    ;;
  *)
    echo "Unsupported profile: $profile (expected: release or release-lto)" >&2
    exit 1
    ;;
esac

cargo build --profile "$profile" --manifest-path "$repo_root/Cargo.toml"
bin="$repo_root/target/$profile/jcode"

if [[ ! -x "$bin" ]]; then
  echo "Release binary not found: $bin" >&2
  exit 1
fi

hash=""
if command -v git >/dev/null 2>&1; then
  if git -C "$repo_root" rev-parse --git-dir >/dev/null 2>&1; then
    hash="$(git -C "$repo_root" rev-parse --short HEAD 2>/dev/null || true)"
    if [[ -n "${hash}" ]] && [[ -n "$(git -C "$repo_root" status --porcelain 2>/dev/null || true)" ]]; then
      hash="${hash}-dirty"
    fi
  fi
fi

if [[ -z "$hash" ]]; then
  hash="$(date +%Y%m%d%H%M%S)"
fi

# Label is now purely informational (there is one publish target, so nothing is
# addressed by label any more), but keeping the profile qualifier makes the
# "which bytes are live" message unambiguous.
version_label="${hash}-${profile}"

# Publish to the single fixed target via stage + atomic rename, so a concurrent
# reader (a daemon about to exec) only ever observes a complete binary.
jcode_home="${JCODE_HOME:-$HOME/.jcode}"
current_dir="$jcode_home/current"
mkdir -p "$current_dir"
staged="$current_dir/.jcode-publish-$$"
trap 'rm -f "$staged"' EXIT
install -m 755 "$bin" "$staged"
mv -f "$staged" "$current_dir/jcode"
trap - EXIT

# Point the launcher at the fixed path.
install_dir="${JCODE_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$install_dir"
ln -sfn "$current_dir/jcode" "$install_dir/jcode"

echo "Published: $current_dir/jcode ($version_label)"
echo "Updated launcher symlink: $install_dir/jcode -> $current_dir/jcode"

# R01 owns live daemon target selection. Reload is therefore explicit opt-in and
# best-effort; JCODE_SKIP_SERVER_RELOAD remains a hard disable for wrappers.
if [ "${JCODE_RELOAD_SERVER:-}" = "1" ] && [ "${JCODE_SKIP_SERVER_RELOAD:-}" != "1" ]; then
  if "$install_dir/jcode" server reload </dev/null >/dev/null 2>&1; then
    echo "Reloaded the running jcode server onto $version_label (if one was active)."
  fi
fi

if ! echo "$PATH" | tr ':' '\n' | grep -qx "$install_dir"; then
  echo ""
  echo "Tip: add $install_dir to PATH if needed."
fi

# Ensure the launcher dir is on PATH for bash, zsh and fish in future shells.
# shellcheck source=scripts/lib/configure_path.sh
. "$(dirname "$0")/lib/configure_path.sh"
jcode_configure_path "$install_dir"
