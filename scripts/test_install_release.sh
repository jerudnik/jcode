#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fake_bin="$tmp/fake-bin"
fake_repo="$tmp/repo"
home="$tmp/home"
mkdir -p "$fake_bin" "$fake_repo/scripts/lib" "$home/.jcode/builds/versions/abc123"
cp "$repo_root/scripts/install_release.sh" "$fake_repo/scripts/"
cp "$repo_root/scripts/lib/configure_path.sh" "$fake_repo/scripts/lib/"
printf 'selfdev-bytes\n' >"$home/.jcode/builds/versions/abc123/jcode"

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

release="$home/.jcode/builds/versions/abc123-release/jcode"
test -x "$release"
test "$(cat "$home/.jcode/builds/versions/abc123/jcode")" = "selfdev-bytes"
test "$(cat "$home/.jcode/builds/current-version")" = "abc123-release"
test "$(cat "$home/.jcode/builds/stable-version")" = "abc123-release"
test "$(readlink "$home/.jcode/builds/current/jcode")" = "$release"
test "$(readlink "$home/.jcode/builds/stable/jcode")" = "$release"
test "$(readlink "$home/.local/bin/jcode")" = "$home/.jcode/builds/current/jcode"

echo "install_release profile-qualified label test: ok"
