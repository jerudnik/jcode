#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/jcode-incremental-policy-test.XXXXXX")
sleeper_pid=""
guarded_pid=""
prune_pid_one=""
prune_pid_two=""
repo_test_target=""
cleanup() {
  if [[ -n "$sleeper_pid" ]]; then
    kill "$sleeper_pid" 2>/dev/null || true
    wait "$sleeper_pid" 2>/dev/null || true
  fi
  if [[ -n "$guarded_pid" ]]; then
    kill "$guarded_pid" 2>/dev/null || true
    wait "$guarded_pid" 2>/dev/null || true
  fi
  for pid in "$prune_pid_one" "$prune_pid_two"; do
    if [[ -n "$pid" ]]; then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    fi
  done
  rm -rf "$tmp"
}
trap cleanup EXIT INT TERM

fail() {
  printf 'test_incremental_policy: FAIL: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local haystack="$1" needle="$2"
  [[ "$haystack" == *"$needle"* ]] ||
    fail "expected output to contain: $needle; output: ${haystack//$'\n'/ | }"
}

fake_bin="$tmp/bin"
mkdir -p "$fake_bin"
printf '#!%s\n' "$BASH" >"$fake_bin/cargo"
cat >>"$fake_bin/cargo" <<'EOF'
printf 'FAKE_CARGO_INCREMENTAL=%s\n' "${CARGO_INCREMENTAL-<unset>}"
printf 'FAKE_CARGO_ARGS=%s\n' "$*"
if [[ -n "${FAKE_CARGO_SENTINEL:-}" ]]; then
  touch "$FAKE_CARGO_SENTINEL"
fi
if [[ -n "${FAKE_CARGO_BLOCK_UNTIL:-}" ]]; then
  while [[ ! -e "$FAKE_CARGO_BLOCK_UNTIL" ]]; do sleep 0.05; done
fi
EOF
chmod +x "$fake_bin/cargo"

run_wrapper() {
  env \
    PATH="$fake_bin:$PATH" \
    JCODE_REMOTE_CARGO=0 \
    JCODE_DEV_SCCACHE=0 \
    JCODE_ENABLE_PARALLEL_FRONTEND=0 \
    JCODE_INCREMENTAL_PRUNE=off \
    JCODE_BUILD_JOBS=1 \
    bash "$repo_root/scripts/dev_cargo.sh" "$@" 2>&1
}

output=$(run_wrapper check --workspace)
assert_contains "$output" 'FAKE_CARGO_INCREMENTAL=0'
assert_contains "$output" 'FAKE_CARGO_ARGS=check --workspace'

output=$(run_wrapper -Z unstable-options check)
assert_contains "$output" 'FAKE_CARGO_INCREMENTAL=0'

output=$(run_wrapper test --profile selfdev --lib)
assert_contains "$output" 'FAKE_CARGO_INCREMENTAL=0'

output=$(run_wrapper build --profile selfdev)
assert_contains "$output" 'FAKE_CARGO_INCREMENTAL=<unset>'

output=$(CARGO_INCREMENTAL=1 run_wrapper check)
assert_contains "$output" 'FAKE_CARGO_INCREMENTAL=1'

locked_target="$tmp/locked-target"
mkdir -p "$locked_target/.jcode-incremental-prune.lock"
printf '%s\n' "$$" >"$locked_target/.jcode-incremental-prune.lock/pid"
set +e
output=$(
  env \
    PATH="$fake_bin:$PATH" \
    CARGO_TARGET_DIR="$locked_target" \
    FAKE_CARGO_SENTINEL="$tmp/cargo-started" \
    JCODE_REMOTE_CARGO=0 \
    JCODE_DEV_SCCACHE=0 \
    JCODE_ENABLE_PARALLEL_FRONTEND=0 \
    JCODE_INCREMENTAL_PRUNE=auto \
    JCODE_INCREMENTAL_PRUNE_ALLOW_EXTERNAL=1 \
    JCODE_INCREMENTAL_PRUNE_LOCK_WAIT_SECONDS=1 \
    JCODE_BUILD_JOBS=1 \
    bash "$repo_root/scripts/dev_cargo.sh" build 2>&1
)
status=$?
set -e
[[ "$status" -eq 75 ]] || fail "wrapper did not fail closed on prune lock timeout: status=$status output=$output"
[[ ! -e "$tmp/cargo-started" ]] || fail 'Cargo started while another prune held the lock'
rm -rf "$locked_target/.jcode-incremental-prune.lock"

external_target="$tmp/external-wrapper-target"
mkdir -p "$external_target/debug/incremental/preserved"
printf preserved >"$external_target/debug/incremental/preserved/data"
output=$(
  env \
    PATH="$fake_bin:$PATH" \
    CARGO_TARGET_DIR="$external_target" \
    JCODE_REMOTE_CARGO=0 \
    JCODE_DEV_SCCACHE=0 \
    JCODE_ENABLE_PARALLEL_FRONTEND=0 \
    JCODE_INCREMENTAL_PRUNE=auto \
    JCODE_BUILD_JOBS=1 \
    bash "$repo_root/scripts/dev_cargo.sh" build 2>&1
)
assert_contains "$output" 'skipping automatic incremental pruning outside the repository'
[[ -f "$external_target/debug/incremental/preserved/data" ]] || fail 'automatic pruning modified an external target without opt-in'

local_test_repo="$tmp/local-wrapper-repo"
symlink_external="$tmp/symlink-external"
mkdir -p "$local_test_repo/scripts" "$symlink_external"
cp \
  "$repo_root/scripts/dev_cargo.sh" \
  "$repo_root/scripts/prune_incremental.sh" \
  "$repo_root/scripts/remote_config.sh" \
  "$local_test_repo/scripts/"
ln -s "$symlink_external" "$local_test_repo/repo-link"
output=$(
  env \
    PATH="$fake_bin:$PATH" \
    JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
    JCODE_REMOTE_CARGO=0 \
    JCODE_DEV_SCCACHE=0 \
    JCODE_ENABLE_PARALLEL_FRONTEND=0 \
    JCODE_INCREMENTAL_PRUNE=auto \
    JCODE_BUILD_JOBS=1 \
    bash "$local_test_repo/scripts/dev_cargo.sh" \
      build --target-dir repo-link/new-target 2>&1
)
assert_contains "$output" 'skipping automatic incremental pruning outside the repository'
[[ ! -e "$symlink_external/new-target" ]] || fail 'automatic pruning followed an in-repository symlink outside the repository'

stuck_reclaim_target="$tmp/stuck-reclaim-target"
stuck_reclaim="$stuck_reclaim_target/.jcode-incremental-prune.reclaim"
stuck_output="$tmp/stuck-reclaim.out"
mkdir -p "$stuck_reclaim"
printf '%s\n' 99999999 >"$stuck_reclaim/pid"
printf '%s\n' abandoned-reclaimer >"$stuck_reclaim/token"
JCODE_INCREMENTAL_PRUNE_LOCK_WAIT_SECONDS=1 \
  bash "$repo_root/scripts/prune_incremental.sh" \
    --target-dir "$stuck_reclaim_target" --profile debug --cap-gib 1 \
    --ignore-active-processes --force --apply --quiet >"$stuck_output" 2>&1 &
prune_pid_one=$!
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
  kill -0 "$prune_pid_one" 2>/dev/null || break
  sleep 0.1
done
if kill -0 "$prune_pid_one" 2>/dev/null; then
  kill "$prune_pid_one" 2>/dev/null || true
  wait "$prune_pid_one" 2>/dev/null || true
  prune_pid_one=""
  fail 'abandoned reclaim guard caused an unbounded retry loop'
fi
set +e
wait "$prune_pid_one"
status=$?
set -e
prune_pid_one=""
[[ "$status" -eq 75 ]] || fail "abandoned reclaim guard did not fail closed: status=$status output=$(cat "$stuck_output")"
[[ ! -e "$stuck_reclaim_target/.jcode-incremental-prune.lock" ]] || fail 'reclaim timeout left a main prune lock behind'

stale_target="$tmp/stale-target"
stale_lock="$stale_target/.jcode-incremental-prune.lock"
stale_gate="$tmp/start-stale-recovery"
mkdir -p "$stale_lock"
printf '%s\n' 99999999 >"$stale_lock/pid"
printf '%s\n' stale-owner >"$stale_lock/token"
for slot in one two; do
  (
    while [[ ! -e "$stale_gate" ]]; do sleep 0.01; done
    JCODE_INCREMENTAL_PRUNE_TEST_ROOT="$stale_target" \
    JCODE_INCREMENTAL_PRUNE_TEST_HOLD_SECONDS=0.3 \
    JCODE_INCREMENTAL_PRUNE_LOCK_WAIT_SECONDS=3 \
      bash "$repo_root/scripts/prune_incremental.sh" \
        --target-dir "$stale_target" --profile debug --cap-gib 1 \
        --ignore-active-processes --force --apply --quiet
  ) &
  if [[ "$slot" == "one" ]]; then
    prune_pid_one=$!
  else
    prune_pid_two=$!
  fi
done
touch "$stale_gate"
wait "$prune_pid_one"
prune_pid_one=""
wait "$prune_pid_two"
prune_pid_two=""
[[ ! -e "$stale_lock" ]] || fail 'stale recovery left the prune lock behind'
[[ ! -e "$stale_target/.jcode-incremental-prune.reclaim" ]] || fail 'stale recovery left the reclaim guard behind'

guarded_target="$tmp/guarded-target"
guarded_release="$tmp/release-guarded-cargo"
env \
  PATH="$fake_bin:$PATH" \
  FAKE_CARGO_SENTINEL="$tmp/guarded-cargo-started" \
  FAKE_CARGO_BLOCK_UNTIL="$guarded_release" \
  JCODE_REMOTE_CARGO=0 \
  JCODE_DEV_SCCACHE=0 \
  JCODE_ENABLE_PARALLEL_FRONTEND=0 \
  JCODE_INCREMENTAL_PRUNE=auto \
  JCODE_INCREMENTAL_PRUNE_ALLOW_EXTERNAL=1 \
  JCODE_BUILD_JOBS=1 \
  bash "$repo_root/scripts/dev_cargo.sh" build --target-dir "$guarded_target" >/dev/null 2>&1 &
guarded_pid=$!
guarded_marker="$guarded_target/.jcode-active-builds/$guarded_pid"
for _ in 1 2 3 4 5 6 7 8 9 10; do
  [[ -e "$tmp/guarded-cargo-started" ]] && break
  sleep 0.1
done
[[ -e "$tmp/guarded-cargo-started" ]] || fail 'guarded Cargo did not start'
[[ -f "$guarded_marker" ]] || fail 'wrapper did not retain an active-build marker'
mkdir -p "$guarded_target/debug/incremental/old"
printf guarded >"$guarded_target/debug/incremental/old/data"
bash "$repo_root/scripts/prune_incremental.sh" \
  --target-dir "$guarded_target" --profile debug --cap-gib 0.000001 \
  --force --apply >/dev/null 2>&1
[[ -d "$guarded_target/debug/incremental/old" ]] || fail 'pruner deleted incremental data during a guarded build'
touch "$guarded_release"
wait "$guarded_pid"
guarded_pid=""
[[ ! -e "$guarded_marker" ]] || fail 'wrapper left a stale active-build marker'

printf '#!%s\n' "$BASH" >"$fake_bin/ssh"
cat >>"$fake_bin/ssh" <<'EOF'
printf '%s\n' "${!#}" >>"$FAKE_SSH_LOG"
exit 0
EOF
printf '#!%s\n' "$BASH" >"$fake_bin/rsync"
cat >>"$fake_bin/rsync" <<'EOF'
if [[ -n "${FAKE_RSYNC_LOG:-}" ]]; then
  printf '%s\n' "$*" >>"$FAKE_RSYNC_LOG"
fi
if [[ "$*" == *"--files-from=-"* ]] && [[ -n "${FAKE_RSYNC_STDIN_LOG:-}" ]]; then
  cat >>"$FAKE_RSYNC_STDIN_LOG"
fi
if [[ "${FAKE_RSYNC_CREATE_DEST:-0}" == "1" ]]; then
  destination="${!#}"
  mkdir -p "$(dirname "$destination")"
  touch "$destination"
fi
exit 0
EOF
chmod +x "$fake_bin/ssh" "$fake_bin/rsync"

stateful_bin="$tmp/stateful-bin"
mkdir -p "$stateful_bin"
cp "$fake_bin/cargo" "$stateful_bin/cargo"
printf '#!%s\n' "$BASH" >"$stateful_bin/sh"
cat >>"$stateful_bin/sh" <<'EOF'
if [[ "${1:-}" == "-lc" ]]; then
  shift
  exec /bin/bash -c "$1"
fi
exec /bin/sh "$@"
EOF
printf '#!%s\n' "$BASH" >"$stateful_bin/ssh"
cat >>"$stateful_bin/ssh" <<'EOF'
command="${!#}"
if [[ -n "${FAKE_SSH_LOG:-}" ]]; then
  printf '%s\n' "$command" >>"$FAKE_SSH_LOG"
fi
PATH="$FAKE_REMOTE_PATH:$PATH" /bin/bash -c "$command"
EOF
printf '#!%s\n' "$BASH" >"$stateful_bin/rsync"
cat >>"$stateful_bin/rsync" <<'EOF'
args=("$@")
source_path="${args[$((${#args[@]} - 2))]}"
destination="${args[$((${#args[@]} - 1))]}"
if [[ -n "${FAKE_RSYNC_LOG:-}" ]]; then
  printf '%s\n' "$*" >>"$FAKE_RSYNC_LOG"
fi
if [[ "$*" == *"--files-from=-"* ]]; then
  cat >/dev/null
fi
if [[ -f "$source_path" && "$destination" == *:* ]]; then
  destination_path="${destination#*:}"
  if [[ "$destination_path" == */ ]]; then
    destination_path+="$(basename "$source_path")"
  fi
  mkdir -p "$(dirname "$destination_path")"
  cp "$source_path" "$destination_path"
fi
EOF
chmod +x "$stateful_bin/cargo" "$stateful_bin/sh" "$stateful_bin/ssh" "$stateful_bin/rsync"

ssh_log="$tmp/ssh.log"
FAKE_SSH_LOG="$ssh_log" \
JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
JCODE_REMOTE_SSH_BIN="$fake_bin/ssh" \
JCODE_REMOTE_RSYNC_BIN="$fake_bin/rsync" \
  bash "$repo_root/scripts/remote_build.sh" \
    --host fake-builder --remote-dir /tmp/jcode-policy-test \
    --no-sync --no-sync-back check --workspace >/dev/null

grep -q 'CARGO_INCREMENTAL=0' "$ssh_log" || fail 'remote check did not disable incrementality'
grep -Fq 'cargo\ check' "$ssh_log" || fail 'remote check command was not forwarded'
grep -Fq '/nix/var/nix/profiles/default/bin/nix' "$ssh_log" || fail 'remote command omitted Nix recovery'

metadata_repo="$tmp/remote-metadata-repo"
mkdir -p "$metadata_repo/scripts" "$metadata_repo/.jcode"
cp "$repo_root/scripts/remote_build.sh" "$repo_root/scripts/remote_config.sh" "$metadata_repo/scripts/"
printf '# project prompt\n' >"$metadata_repo/.jcode/prompt-overlay.md"
git -C "$metadata_repo" init -q
git -C "$metadata_repo" add scripts .jcode/prompt-overlay.md
git -C "$metadata_repo" \
  -c user.name=Jcode -c user.email=jcode@example.invalid -c commit.gpgsign=false \
  commit -qm initial
metadata_hash=$(git -C "$metadata_repo" rev-parse --short HEAD)
rsync_log="$tmp/rsync.log"
: >"$ssh_log"
: >"$rsync_log"
jcode_rsync_stdin_log="$tmp/jcode-rsync-stdin.log"
: >"$jcode_rsync_stdin_log"
FAKE_SSH_LOG="$ssh_log" \
FAKE_RSYNC_LOG="$rsync_log" \
FAKE_RSYNC_STDIN_LOG="$jcode_rsync_stdin_log" \
JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
JCODE_REMOTE_SSH_BIN="$fake_bin/ssh" \
JCODE_REMOTE_RSYNC_BIN="$fake_bin/rsync" \
  bash "$metadata_repo/scripts/remote_build.sh" \
    --host fake-builder --remote-dir /tmp/jcode-policy-test \
    --no-sync-back check >/dev/null

grep -Fq "JCODE_BUILD_GIT_HASH=$metadata_hash" "$ssh_log" || fail 'synced remote build did not force git metadata refresh'
grep -Fq 'JCODE_BUILD_GIT_DIRTY=0' "$ssh_log" || fail 'synced remote build did not forward clean source state'
grep -Eq -- '--delete .* fake-builder:/tmp/jcode-policy-test/.jcode/' "$rsync_log" || fail 'remote build did not clear stale .jcode inputs'
grep -Fq -- '--from0 --files-from=-' "$rsync_log" || fail 'remote build did not use a tracked-only .jcode sync'
tr '\0' '\n' <"$jcode_rsync_stdin_log" | grep -Fxq '.jcode/prompt-overlay.md' || fail 'remote build omitted tracked .jcode inputs'

fingerprint_remote="$tmp/fingerprint-remote"
fingerprint_cargo_sentinel="$tmp/fingerprint-cargo-started"
stateful_output=$(
  FAKE_CARGO_SENTINEL="$fingerprint_cargo_sentinel" \
  FAKE_REMOTE_PATH="$stateful_bin" \
  JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
  JCODE_REMOTE_SSH_BIN="$stateful_bin/ssh" \
  JCODE_REMOTE_RSYNC_BIN="$stateful_bin/rsync" \
    bash "$metadata_repo/scripts/remote_build.sh" \
      --host fake-builder --remote-dir "$fingerprint_remote" \
      --no-sync-back check 2>&1
)
assert_contains "$stateful_output" 'remote_build: verified source fingerprint'
[[ -e "$fingerprint_cargo_sentinel" ]] || fail 'fresh fingerprint verification did not reach Cargo'

rm -f "$fingerprint_cargo_sentinel"
stateful_output=$(
  FAKE_CARGO_SENTINEL="$fingerprint_cargo_sentinel" \
  FAKE_REMOTE_PATH="$stateful_bin" \
  JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
  JCODE_REMOTE_SSH_BIN="$stateful_bin/ssh" \
  JCODE_REMOTE_RSYNC_BIN="$stateful_bin/rsync" \
    bash "$metadata_repo/scripts/remote_build.sh" \
      --host fake-builder --remote-dir "$fingerprint_remote" \
      --no-sync --no-sync-back check 2>&1
)
assert_contains "$stateful_output" 'remote_build: reusing source fingerprint'
assert_contains "$stateful_output" 'age '
[[ -e "$fingerprint_cargo_sentinel" ]] || fail 'matching retained fingerprint did not reach Cargo'

printf '# local dirty change\n' >>"$metadata_repo/.jcode/prompt-overlay.md"
rm -f "$fingerprint_cargo_sentinel"
set +e
stateful_output=$(
  FAKE_CARGO_SENTINEL="$fingerprint_cargo_sentinel" \
  FAKE_REMOTE_PATH="$stateful_bin" \
  JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
  JCODE_REMOTE_SSH_BIN="$stateful_bin/ssh" \
  JCODE_REMOTE_RSYNC_BIN="$stateful_bin/rsync" \
    bash "$metadata_repo/scripts/remote_build.sh" \
      --host fake-builder --remote-dir "$fingerprint_remote" \
      --no-sync --no-sync-back check 2>&1
)
status=$?
set -e
[[ "$status" -eq 86 ]] || fail "fingerprint mismatch did not fail closed: status=$status output=$stateful_output"
assert_contains "$stateful_output" 'remote_build: source fingerprint mismatch'
assert_contains "$stateful_output" 'refusing to run Cargo'
[[ ! -e "$fingerprint_cargo_sentinel" ]] || fail 'Cargo ran after a source fingerprint mismatch'

set +e
fmt_output=$(
  env \
    PATH="$fake_bin:$PATH" \
    JCODE_REMOTE_CARGO=1 \
    JCODE_REMOTE_HOST=fake-builder \
    JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
    JCODE_DEV_SCCACHE=0 \
    JCODE_ENABLE_PARALLEL_FRONTEND=0 \
    JCODE_INCREMENTAL_PRUNE=off \
    JCODE_BUILD_JOBS=1 \
    bash "$repo_root/scripts/dev_cargo.sh" fmt 2>&1
)
status=$?
set -e
[[ "$status" -eq 2 ]] || fail "remote fmt was not refused: status=$status output=$fmt_output"
assert_contains "$fmt_output" 'refusing to run cargo fmt remotely'
assert_contains "$fmt_output" 'JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh fmt'

: >"$ssh_log"
FAKE_SSH_LOG="$ssh_log" \
JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
JCODE_REMOTE_SSH_BIN="$fake_bin/ssh" \
JCODE_REMOTE_RSYNC_BIN="$fake_bin/rsync" \
  bash "$repo_root/scripts/remote_build.sh" \
    --host fake-builder --remote-dir /tmp/jcode-policy-test \
    --no-sync --no-sync-back -Z unstable-options check --workspace >/dev/null

grep -Fq 'cargo\ -Z\ unstable-options\ check\ --workspace' "$ssh_log" || fail 'remote Cargo global options were reordered'

: >"$ssh_log"
FAKE_SSH_LOG="$ssh_log" \
JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
JCODE_REMOTE_SSH_BIN="$fake_bin/ssh" \
JCODE_REMOTE_RSYNC_BIN="$fake_bin/rsync" \
  bash "$repo_root/scripts/remote_build.sh" \
    --host fake-builder --remote-dir /tmp/jcode-policy-test \
    --no-sync --no-sync-back +nightly check >/dev/null

grep -q 'CARGO_INCREMENTAL=0' "$ssh_log" || fail 'remote toolchain check did not disable incrementality'
grep -Fq 'cargo\ +nightly\ check' "$ssh_log" || fail 'remote Cargo toolchain selector was misclassified'

: >"$ssh_log"
FAKE_SSH_LOG="$ssh_log" \
CARGO_INCREMENTAL=1 \
JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
JCODE_REMOTE_SSH_BIN="$fake_bin/ssh" \
JCODE_REMOTE_RSYNC_BIN="$fake_bin/rsync" \
  bash "$repo_root/scripts/remote_build.sh" \
    --host fake-builder --remote-dir /tmp/jcode-policy-test \
    --no-sync --no-sync-back check >/dev/null

grep -q 'CARGO_INCREMENTAL=1' "$ssh_log" || fail 'remote build did not preserve explicit incrementality'

: >"$ssh_log"
printf 'JCODE_INCREMENTAL_POLICY=off\n' >"$tmp/remote-config"
FAKE_SSH_LOG="$ssh_log" \
JCODE_INCREMENTAL_POLICY=profile-default \
JCODE_REMOTE_CONFIG="$tmp/remote-config" \
JCODE_REMOTE_SSH_BIN="$fake_bin/ssh" \
JCODE_REMOTE_RSYNC_BIN="$fake_bin/rsync" \
  bash "$repo_root/scripts/remote_build.sh" \
    --host fake-builder --remote-dir /tmp/jcode-policy-test \
    --no-sync --no-sync-back check >/dev/null

if grep -q 'CARGO_INCREMENTAL=' "$ssh_log"; then
  fail 'remote build ignored JCODE_INCREMENTAL_POLICY=profile-default'
fi

remote_test_repo="$tmp/remote-test-repo"
mkdir -p "$remote_test_repo/scripts"
remote_test_repo=$(cd "$remote_test_repo" && pwd)
cp "$repo_root/scripts/remote_build.sh" "$repo_root/scripts/remote_config.sh" "$remote_test_repo/scripts/"
git -C "$remote_test_repo" init -q
git -C "$remote_test_repo" add scripts
git -C "$remote_test_repo" \
  -c user.name=Jcode -c user.email=jcode@example.invalid -c commit.gpgsign=false \
  commit -qm initial
target_dir_arg="$tmp/remote-absolute-target"
repo_test_target="$remote_test_repo/target"
: >"$ssh_log"
: >"$rsync_log"
FAKE_SSH_LOG="$ssh_log" \
FAKE_RSYNC_LOG="$rsync_log" \
FAKE_RSYNC_CREATE_DEST=1 \
JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
JCODE_REMOTE_SSH_BIN="$fake_bin/ssh" \
JCODE_REMOTE_RSYNC_BIN="$fake_bin/rsync" \
  bash "$remote_test_repo/scripts/remote_build.sh" \
    --host fake-builder --remote-dir /tmp/jcode-policy-test \
    --no-sync build --target-dir "$target_dir_arg" >/dev/null

grep -Fq "cargo\\ build\\ --target-dir\\ $target_dir_arg" "$ssh_log" || fail 'remote custom target command was not forwarded'
grep -Fq "fake-builder:$target_dir_arg/debug/jcode" "$rsync_log" || fail 'remote sync-back ignored absolute target directory'
grep -Fq "$repo_test_target/debug/jcode" "$rsync_log" || fail 'absolute remote target escaped the safe local sync directory'

target_dir_arg="custom-target"
repo_test_target="$remote_test_repo/$target_dir_arg"
: >"$ssh_log"
: >"$rsync_log"
FAKE_SSH_LOG="$ssh_log" \
FAKE_RSYNC_LOG="$rsync_log" \
FAKE_RSYNC_CREATE_DEST=1 \
JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
JCODE_REMOTE_SSH_BIN="$fake_bin/ssh" \
JCODE_REMOTE_RSYNC_BIN="$fake_bin/rsync" \
  bash "$remote_test_repo/scripts/remote_build.sh" \
    --host fake-builder --remote-dir /tmp/jcode-policy-test \
    --no-sync --target-dir "$target_dir_arg" build >/dev/null

grep -Fq "fake-builder:/tmp/jcode-policy-test/$target_dir_arg/debug/jcode" "$rsync_log" || fail 'remote sync-back ignored relative target directory'
grep -Fq "$repo_test_target/debug/jcode" "$rsync_log" || fail 'local sync-back ignored relative target directory'

set +e
output=$(
  FAKE_SSH_LOG="$ssh_log" \
  FAKE_RSYNC_LOG="$rsync_log" \
  JCODE_REMOTE_CONFIG="$tmp/missing-remote-config" \
  JCODE_REMOTE_SSH_BIN="$fake_bin/ssh" \
  JCODE_REMOTE_RSYNC_BIN="$fake_bin/rsync" \
    bash "$remote_test_repo/scripts/remote_build.sh" \
      --host fake-builder --remote-dir /tmp/jcode-policy-test \
      --no-sync --target-dir ../escape-target build 2>&1
)
status=$?
set -e
[[ "$status" -eq 2 ]] || fail "relative target traversal was not rejected: status=$status output=$output"
assert_contains "$output" 'must not contain .. path components'
[[ ! -e "$tmp/escape-target" ]] || fail 'relative target traversal wrote outside the local repository'

config_precedence="$tmp/config-precedence.env"
config_vars=(
  JCODE_INCREMENTAL_POLICY
  JCODE_REMOTE_CARGO
  JCODE_REMOTE_CARGO_FALLBACK
  JCODE_REMOTE_CONFIG
  JCODE_REMOTE_CONNECT_TIMEOUT
  JCODE_REMOTE_DIR
  JCODE_REMOTE_DOWN_TTL
  JCODE_REMOTE_HOST
  JCODE_REMOTE_RECOVERY_TCP_TIMEOUT
  JCODE_REMOTE_RSYNC_BIN
  JCODE_REMOTE_RSYNC_SSH
  JCODE_REMOTE_SERVER_ALIVE_COUNT_MAX
  JCODE_REMOTE_SERVER_ALIVE_INTERVAL
  JCODE_REMOTE_SSH_BIN
  JCODE_REMOTE_TCP_PROBE
  JCODE_REMOTE_TCP_TIMEOUT
)
: >"$config_precedence"
for name in "${config_vars[@]}"; do
  printf '%s=from-config\n' "$name" >>"$config_precedence"
done
(
  # The loader path and variable names are deliberately dynamic so this one
  # loop exercises the complete supported-variable inventory.
  # shellcheck disable=SC1091
  source "$repo_root/scripts/remote_config.sh"
  for name in "${config_vars[@]}"; do
    # shellcheck disable=SC2030
    printf -v "$name" '%s' "from-env-$name"
    export "${name?}"
  done
  JCODE_REMOTE_CONFIG="$config_precedence"
  export JCODE_REMOTE_CONFIG
  jcode_load_remote_config
  for name in "${config_vars[@]}"; do
    expected="from-env-$name"
    if [[ "$name" == "JCODE_REMOTE_CONFIG" ]]; then
      expected="$config_precedence"
    fi
    [[ "${!name}" == "$expected" ]] || fail "config overwrote explicit $name"
  done
)

# Prove the macOS/Linux process guard fails closed while a repo-local cargo
# process is running. A tiny native blocker named "cargo" avoids compiling the
# jcode workspace and makes pgrep behavior deterministic.
clean_target="$tmp/clean-target"
mkdir -p "$clean_target/x86_64-pc-windows-msvc"
printf keep >"$clean_target/x86_64-pc-windows-msvc/artifact"
cat >"$tmp/cargo-blocker.c" <<'EOF'
#include <unistd.h>
int main(void) {
  sleep(20);
  return 0;
}
EOF
cc "$tmp/cargo-blocker.c" -o "$fake_bin/cargo"
(
  cd "$repo_root"
  exec "$fake_bin/cargo"
) &
# shellcheck disable=SC2031
sleeper_pid=$!
process_path="$PATH"
if [[ -x /usr/bin/pgrep ]]; then
  process_path="/usr/bin:/bin:/usr/sbin:$PATH"
fi
for _ in 1 2 3 4 5; do
  PATH="$process_path" pgrep -x cargo >/dev/null 2>&1 && break
  sleep 0.1
done
PATH="$process_path" pgrep -x cargo >/dev/null 2>&1 || fail 'could not start fake cargo process'
PATH="$process_path" CARGO_TARGET_DIR="$clean_target" JCODE_CLEAN_ACTIVE_WINDOW_MIN=0 \
  bash "$repo_root/scripts/clean_target.sh" --apply >/dev/null 2>&1
[[ -d "$clean_target/x86_64-pc-windows-msvc" ]] || fail 'clean_target removed a cache during an active repo build'
kill "$sleeper_pid" 2>/dev/null || true
wait "$sleeper_pid" 2>/dev/null || true
sleeper_pid=""

# Exercise dry-run and applied pruning on a tiny synthetic cache. Dependency
# artifacts and final binaries must remain untouched.
prune_target="$tmp/prune-target"
mkdir -p \
  "$prune_target/debug/incremental/old" \
  "$prune_target/debug/incremental/new" \
  "$prune_target/debug/deps" \
  "$prune_target/selfdev"
dd if=/dev/urandom of="$prune_target/debug/incremental/old/data" bs=1048576 count=2 2>/dev/null
dd if=/dev/urandom of="$prune_target/debug/incremental/new/data" bs=1048576 count=2 2>/dev/null
printf dependency >"$prune_target/debug/deps/keep"
printf binary >"$prune_target/selfdev/jcode"
touch -t 202001010000 "$prune_target/debug/incremental/old"
touch -t 202101010000 "$prune_target/debug/incremental/new"

output=$(
  bash "$repo_root/scripts/prune_incremental.sh" \
    --target-dir "$prune_target" --profile debug --cap-gib 0.003 --force \
    --ignore-active-processes --apparent-size 2>&1
)
assert_contains "$output" 'would reclaim'
[[ -d "$prune_target/debug/incremental/old" ]] || fail 'dry-run removed old incremental cache'

bash "$repo_root/scripts/prune_incremental.sh" \
  --target-dir "$prune_target" --profile debug --cap-gib 0.003 --force \
  --ignore-active-processes --apparent-size --apply >/dev/null
[[ ! -d "$prune_target/debug/incremental/old" ]] || fail 'applied prune did not remove the oldest cache entry'
[[ -d "$prune_target/debug/incremental/new" ]] || fail 'applied prune removed the newest cache entry'
[[ -f "$prune_target/debug/deps/keep" ]] || fail 'applied prune removed dependency artifacts'
[[ -f "$prune_target/selfdev/jcode" ]] || fail 'applied prune removed the selfdev binary'

printf 'test_incremental_policy: PASS\n'
