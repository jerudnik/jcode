#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fake_bin="$tmp/fake-bin"
fake_repo="$tmp/repo"
home="$tmp/home"
mkdir -p "$fake_bin" "$fake_repo/scripts/lib" "$home/.jcode/current"
cp "$repo_root/scripts/install_release.sh" "$fake_repo/scripts/"
cp "$repo_root/scripts/lib/configure_path.sh" "$fake_repo/scripts/lib/"
# A previously published binary must be replaced in place by the new publish.
printf 'stale-bytes\n' >"$home/.jcode/current/jcode"

cat >"$fake_bin/git" <<EOF
#!/usr/bin/env bash
case "\$*" in
  "rev-parse --show-toplevel") printf '%s\n' "$fake_repo" ;;
  "-C $fake_repo rev-parse --git-dir") printf '%s\n' "$fake_repo/.git" ;;
  "-C $fake_repo rev-parse --short HEAD") printf '%s\n' abc123 ;;
  "-C $fake_repo status --porcelain") ;;
  *) printf 'unexpected git invocation: %s\n' "\$*" >&2; exit 1 ;;
esac
EOF

cat >"$fake_bin/cargo" <<EOF
#!/usr/bin/env bash
mkdir -p "$fake_repo/target/release"
cat > "$fake_repo/target/release/jcode" <<'BIN'
#!/usr/bin/env bash
exit 0
BIN
chmod +x "$fake_repo/target/release/jcode"
EOF
chmod +x "$fake_bin/git" "$fake_bin/cargo"

HOME="$home" \
  PATH="$fake_bin:$home/.local/bin:/usr/bin:/bin" \
  JCODE_SKIP_SERVER_RELOAD=1 \
  bash "$fake_repo/scripts/install_release.sh" --fast >/dev/null

# F20c: exactly one publish target, and the launcher points at it.
published="$home/.jcode/current/jcode"
test -x "$published"
test "$(cat "$published")" != "stale-bytes"
test "$(readlink "$home/.local/bin/jcode")" = "$published"

# No channel/version-store residue may be recreated: those paths are no longer
# read by anything, so writing them would be stale state by construction.
test ! -e "$home/.jcode/builds/stable"
test ! -e "$home/.jcode/builds/current"
test ! -e "$home/.jcode/builds/versions"
test ! -e "$home/.jcode/builds/stable-version"
test ! -e "$home/.jcode/builds/current-version"

# The staged temp must not survive a successful publish.
test -z "$(find "$home/.jcode/current" -name '.jcode-publish-*' -print -quit)"

echo "install_release fixed-path publish test: ok"
