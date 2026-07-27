#!/usr/bin/env bash
# Remote cargo runner (build/test/check/clippy) via SSH + rsync.
#
# Defaults:
# - Config file: ~/.config/jcode/remote-build.env (override with JCODE_REMOTE_CONFIG)
# - Host: JCODE_REMOTE_HOST from env/config, or --host
# - Remote dir: .cache/remote-builds/jcode/<repo-name> (override with JCODE_REMOTE_DIR or --remote-dir)
#
# Examples:
#   scripts/remote_build.sh --release
#   scripts/remote_build.sh test
#   scripts/remote_build.sh check --all-targets
#   scripts/remote_build.sh --host mybox --remote-dir ~/src/jcode test -- --nocapture

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/remote_build.sh [options] [cargo-subcommand] [cargo-args...]

Options:
  -r, --release        Add --release to cargo invocation
  --host HOST          Remote SSH host (default: $JCODE_REMOTE_HOST from env/config; required if unset)
  --remote-dir DIR     Remote project directory (default: $JCODE_REMOTE_DIR or .cache/remote-builds/jcode/<repo-name>)
  --no-sync            Skip rsync upload step
  --sync-back          Force sync-back of built binary after command
  --no-sync-back       Disable sync-back of built binary after command
  -h, --help           Show this help

Behavior:
  - Default cargo subcommand is 'build'
  - Sync-back defaults to ON for 'build', OFF for other subcommands
  - For build sync-back, copies target/{debug|release}/<artifact> from remote to local
    (artifact defaults to 'jcode', or '--bin <name>' when provided)
  - Default config file is ~/.config/jcode/remote-build.env
EOF
}

LOCAL_DIR="$(cd "$(dirname "$0")/.." && pwd)"
REPO_NAME="$(basename "$LOCAL_DIR")"

# shellcheck source=scripts/remote_config.sh
source "$LOCAL_DIR/scripts/remote_config.sh"
jcode_load_remote_config

REMOTE="${JCODE_REMOTE_HOST:-}"
REMOTE_DIR="${JCODE_REMOTE_DIR:-.cache/remote-builds/jcode/${REPO_NAME}}"
SSH_BIN="${JCODE_REMOTE_SSH_BIN:-ssh}"
RSYNC_BIN="${JCODE_REMOTE_RSYNC_BIN:-rsync}"

SYNC_SOURCE=1
SYNC_BACK_MODE="auto" # auto|always|never
RELEASE=0
SUBCOMMAND="build"
SUBCOMMAND_SET=0
EXPECT_GLOBAL_VALUE=0
GLOBAL_ARGS=()
POSITIONAL=()

remote_connect_timeout() {
    local value="${JCODE_REMOTE_CONNECT_TIMEOUT:-5}"
    if [[ ! "$value" =~ ^[0-9]+$ || "$value" -lt 1 ]]; then
        value=5
    fi
    printf '%s\n' "$value"
}

while [[ $# -gt 0 ]]; do
    if [[ "$SUBCOMMAND_SET" -eq 0 && "$EXPECT_GLOBAL_VALUE" -eq 1 ]]; then
        GLOBAL_ARGS+=("$1")
        EXPECT_GLOBAL_VALUE=0
        shift
        continue
    fi
    case "$1" in
        -r|--release)
            RELEASE=1
            shift
            ;;
        --host)
            [[ $# -lt 2 ]] && { echo "error: --host requires a value" >&2; exit 2; }
            REMOTE="$2"
            shift 2
            ;;
        --remote-dir)
            [[ $# -lt 2 ]] && { echo "error: --remote-dir requires a value" >&2; exit 2; }
            REMOTE_DIR="$2"
            shift 2
            ;;
        --no-sync)
            SYNC_SOURCE=0
            shift
            ;;
        --sync-back)
            SYNC_BACK_MODE="always"
            shift
            ;;
        --no-sync-back)
            SYNC_BACK_MODE="never"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            POSITIONAL+=("$@")
            break
            ;;
        *)
            if [[ "$SUBCOMMAND_SET" -eq 0 ]]; then
                case "$1" in
                    +*)
                        GLOBAL_ARGS+=("$1")
                        ;;
                    -C|-Z|--color|--config|--manifest-path|--target-dir)
                        GLOBAL_ARGS+=("$1")
                        EXPECT_GLOBAL_VALUE=1
                        ;;
                    -C*|-Z*|--color=*|--config=*|--manifest-path=*|--target-dir=*|-*)
                        GLOBAL_ARGS+=("$1")
                        ;;
                    *)
                        SUBCOMMAND="$1"
                        SUBCOMMAND_SET=1
                        ;;
                esac
            else
                POSITIONAL+=("$1")
            fi
            shift
            ;;
    esac
done

if [[ "$EXPECT_GLOBAL_VALUE" -eq 1 ]]; then
    echo "error: Cargo global option requires a value" >&2
    exit 2
fi

if [[ "$REMOTE_DIR" == *" "* ]]; then
    echo "error: remote dir cannot contain spaces: $REMOTE_DIR" >&2
    exit 2
fi

if [[ -z "$REMOTE" ]]; then
    echo "error: remote host not configured; set JCODE_REMOTE_HOST or pass --host HOST" >&2
    exit 2
fi

for bin in "$SSH_BIN" "$RSYNC_BIN"; do
    if ! command -v "$bin" >/dev/null 2>&1; then
        echo "error: required binary not found: $bin" >&2
        exit 2
    fi
done

SSH_CONNECT_TIMEOUT="$(remote_connect_timeout)"
SSH_SERVER_ALIVE_INTERVAL="${JCODE_REMOTE_SERVER_ALIVE_INTERVAL:-10}"
SSH_SERVER_ALIVE_COUNT_MAX="${JCODE_REMOTE_SERVER_ALIVE_COUNT_MAX:-1}"
SSH_OPTS=(
    -o BatchMode=yes
    -o ConnectTimeout="$SSH_CONNECT_TIMEOUT"
    -o ServerAliveInterval="$SSH_SERVER_ALIVE_INTERVAL"
    -o ServerAliveCountMax="$SSH_SERVER_ALIVE_COUNT_MAX"
)
RSYNC_SSH_COMMAND="${JCODE_REMOTE_RSYNC_SSH:-$SSH_BIN -o BatchMode=yes -o ConnectTimeout=$SSH_CONNECT_TIMEOUT -o ServerAliveInterval=$SSH_SERVER_ALIVE_INTERVAL -o ServerAliveCountMax=$SSH_SERVER_ALIVE_COUNT_MAX}"

remote_ssh() {
    "$SSH_BIN" "${SSH_OPTS[@]}" "$REMOTE" "$@"
}

CARGO_CMD=(cargo)
if [[ "${#GLOBAL_ARGS[@]}" -gt 0 ]]; then
    CARGO_CMD+=("${GLOBAL_ARGS[@]}")
fi
CARGO_CMD+=("$SUBCOMMAND")
if [[ "$RELEASE" -eq 1 ]]; then
    CARGO_CMD+=(--release)
fi
if [[ "${#POSITIONAL[@]}" -gt 0 ]]; then
    CARGO_CMD+=("${POSITIONAL[@]}")
fi

remote_incremental=""
if [[ ${CARGO_INCREMENTAL+x} ]]; then
    remote_incremental="$CARGO_INCREMENTAL"
else
    incremental_policy="${JCODE_INCREMENTAL_POLICY:-verification-off}"
    case "$incremental_policy" in
        verification-off|auto)
            case "$SUBCOMMAND" in
                test|check|clippy|bench|doc|rustdoc)
                    remote_incremental=0
                    ;;
            esac
            ;;
        profile-default|keep|on)
            ;;
        off|0|false|no)
            remote_incremental=0
            ;;
        *)
            printf 'error: unsupported JCODE_INCREMENTAL_POLICY=%s (expected verification-off|profile-default|off)\n' "$incremental_policy" >&2
            exit 2
            ;;
    esac
fi

sync_back=0
case "$SYNC_BACK_MODE" in
    always) sync_back=1 ;;
    never) sync_back=0 ;;
    auto)
        if [[ "$SUBCOMMAND" == "build" ]]; then
            sync_back=1
        fi
        ;;
esac

profile_name=""
for ((i=0; i<${#POSITIONAL[@]}; i++)); do
    case "${POSITIONAL[$i]}" in
        --profile)
            if [[ $((i + 1)) -lt ${#POSITIONAL[@]} ]]; then
                profile_name="${POSITIONAL[$((i + 1))]}"
            fi
            ;;
        --profile=*)
            profile_name="${POSITIONAL[$i]#--profile=}"
            ;;
    esac
done

if [[ "$RELEASE" -eq 1 || "$profile_name" == "release" ]]; then
    build_mode="release"
elif [[ -n "$profile_name" && "$profile_name" != "dev" ]]; then
    build_mode="$profile_name"
else
    build_mode="debug"
fi

artifact_name="jcode"
if [[ "$SUBCOMMAND" == "build" ]]; then
    for ((i=0; i<${#POSITIONAL[@]}; i++)); do
        if [[ "${POSITIONAL[$i]}" == "--bin" && $((i + 1)) -lt ${#POSITIONAL[@]} ]]; then
            artifact_name="${POSITIONAL[$((i + 1))]}"
            break
        fi
    done
fi

target_dir_value="target"
expect_target_dir=0
scan_target_dir_args() {
    local arg
    for arg in "$@"; do
        if [[ "$expect_target_dir" -eq 1 ]]; then
            target_dir_value="$arg"
            expect_target_dir=0
            continue
        fi
        case "$arg" in
            --) return 0 ;;
            --target-dir) expect_target_dir=1 ;;
            --target-dir=*) target_dir_value="${arg#--target-dir=}" ;;
        esac
    done
}
if [[ "${#GLOBAL_ARGS[@]}" -gt 0 ]]; then
    scan_target_dir_args "${GLOBAL_ARGS[@]}"
fi
if [[ "${#POSITIONAL[@]}" -gt 0 ]]; then
    scan_target_dir_args "${POSITIONAL[@]}"
fi

BINARY_PATH="${target_dir_value%/}/${build_mode}/${artifact_name}"
if [[ "$target_dir_value" == /* ]]; then
    REMOTE_BINARY_PATH="$BINARY_PATH"
    LOCAL_BINARY_PATH="$LOCAL_DIR/target/${build_mode}/${artifact_name}"
elif [[ "/$target_dir_value/" == *"/../"* ]]; then
    printf 'error: relative --target-dir must not contain .. path components: %s\n' "$target_dir_value" >&2
    exit 2
else
    REMOTE_BINARY_PATH="$REMOTE_DIR/$BINARY_PATH"
    LOCAL_BINARY_PATH="$LOCAL_DIR/$BINARY_PATH"
fi

local_git_hash=""
local_git_date=""
local_git_tag=""
local_git_dirty="0"
local_changelog_raw=""
if command -v git >/dev/null 2>&1 && git -C "$LOCAL_DIR" rev-parse --git-dir >/dev/null 2>&1; then
    local_git_hash="$(git -C "$LOCAL_DIR" rev-parse --short HEAD 2>/dev/null || true)"
    local_git_date="$(git -C "$LOCAL_DIR" log -1 --format=%ci 2>/dev/null || true)"
    local_git_tag="$(git -C "$LOCAL_DIR" describe --tags --always 2>/dev/null || true)"
    local_changelog_raw="$(git -C "$LOCAL_DIR" log -700 --format='%h|%ct|%D|%s' 2>/dev/null || true)"
    if [[ -n "$(git -C "$LOCAL_DIR" status --porcelain 2>/dev/null || true)" ]]; then
        local_git_dirty="1"
    fi
fi

echo "=== Remote Cargo on $REMOTE ==="
echo "Local:   $LOCAL_DIR"
echo "Remote:  $REMOTE_DIR"
echo "Command: ${CARGO_CMD[*]}"
echo "Mode:    $build_mode"
echo "SSH timeout: ${SSH_CONNECT_TIMEOUT}s"

echo ""
echo "[0/3] Checking remote SSH..."
if ! preflight_output="$(remote_ssh "printf 'jcode-remote-ok\\n'" 2>&1)"; then
    echo "error: remote host '$REMOTE' is not reachable within ${SSH_CONNECT_TIMEOUT}s" >&2
    echo "$preflight_output" >&2
    echo "hint: set JCODE_REMOTE_CARGO=0 to force local cargo, or fix JCODE_REMOTE_HOST/JCODE_REMOTE_CONNECT_TIMEOUT." >&2
    exit 75
fi

if [[ "$SYNC_SOURCE" -eq 1 ]]; then
    echo ""
    echo "[1/3] Syncing source files..."
    remote_ssh "$(printf 'mkdir -p %q' "$REMOTE_DIR")"
    "$RSYNC_BIN" -avz --delete \
        -e "$RSYNC_SSH_COMMAND" \
        --exclude 'target/' \
        --exclude '.git/' \
        --exclude '*.log' \
        --exclude '.claude/' \
        --exclude '.codex-socktest/' \
        --exclude '.jcode/' \
        --exclude '.tmp/' \
        --exclude '.wrangler/' \
        --exclude 'tmp/' \
        --exclude 'node_modules/' \
        --exclude 'assets/demos/' \
        --exclude 'assets/readme/' \
        "$LOCAL_DIR/" "$REMOTE:$REMOTE_DIR/"

    # Project prompt and skill sources are build inputs, but .jcode may also
    # contain ignored local artifacts. Clear the remote copy, then restore only
    # paths tracked by Git so removed prompt inputs cannot remain stale.
    (
        empty_jcode_dir="$(mktemp -d)"
        trap 'rm -rf "$empty_jcode_dir"' EXIT
        "$RSYNC_BIN" -az --delete \
            -e "$RSYNC_SSH_COMMAND" \
            "$empty_jcode_dir/" "$REMOTE:$REMOTE_DIR/.jcode/"
    )
    if git -C "$LOCAL_DIR" ls-files -- .jcode | grep -q .; then
        git -C "$LOCAL_DIR" ls-files -z -- .jcode | \
            "$RSYNC_BIN" -avz --from0 --files-from=- \
                -e "$RSYNC_SSH_COMMAND" \
                "$LOCAL_DIR/" "$REMOTE:$REMOTE_DIR/"
    fi

    metadata_file="$(mktemp)"
    trap 'rm -f "$metadata_file"' EXIT
    {
        printf 'git_hash=%s\n' "$local_git_hash"
        printf 'git_date=%s\n' "$local_git_date"
        printf 'git_tag=%s\n' "$local_git_tag"
        printf 'git_dirty=%s\n' "$local_git_dirty"
        printf 'changelog_raw<<JCODE_CHANGELOG_EOF\n%s\nJCODE_CHANGELOG_EOF\n' "$local_changelog_raw"
    } > "$metadata_file"
    "$RSYNC_BIN" -avz -e "$RSYNC_SSH_COMMAND" "$metadata_file" "$REMOTE:$REMOTE_DIR/.jcode-build-meta"
else
    echo ""
    echo "[1/3] Skipping source sync (--no-sync)"
fi

printf -v REMOTE_CARGO_CMD '%q ' "${CARGO_CMD[@]}"
REMOTE_ENV=(JCODE_BUILD_METADATA_FILE=.jcode-build-meta)
if [[ "$SYNC_SOURCE" -eq 1 ]]; then
    [[ -n "$local_git_hash" ]] && REMOTE_ENV+=("JCODE_BUILD_GIT_HASH=$local_git_hash")
    [[ -n "$local_git_date" ]] && REMOTE_ENV+=("JCODE_BUILD_GIT_DATE=$local_git_date")
    REMOTE_ENV+=("JCODE_BUILD_GIT_DIRTY=$local_git_dirty")
    REMOTE_ENV+=("JCODE_BUILD_GIT_TAG=$local_git_tag")
fi
if [[ -n "$remote_incremental" ]]; then
    REMOTE_ENV+=("CARGO_INCREMENTAL=$remote_incremental")
fi
printf -v REMOTE_ENV_CMD '%q ' "${REMOTE_ENV[@]}"
printf -v REMOTE_PAYLOAD 'env %s%s' "$REMOTE_ENV_CMD" "$REMOTE_CARGO_CMD"
printf -v REMOTE_INNER_CMD \
    'cd %q && if command -v cargo >/dev/null 2>&1; then %s; elif command -v nix >/dev/null 2>&1; then nix develop . --command %s; elif [ -x /nix/var/nix/profiles/default/bin/nix ]; then /nix/var/nix/profiles/default/bin/nix develop . --command %s; else printf %q >&2; exit 127; fi' \
    "$REMOTE_DIR" \
    "$REMOTE_PAYLOAD" \
    "$REMOTE_PAYLOAD" \
    "$REMOTE_PAYLOAD" \
    'remote_build: cargo and nix are unavailable on the remote host\n'
printf -v REMOTE_RUN_CMD 'sh -lc %q' "$REMOTE_INNER_CMD"
echo ""
echo "[2/3] Running on remote..."
remote_ssh "$REMOTE_RUN_CMD 2>&1"

echo ""
if [[ "$sync_back" -eq 1 ]]; then
    printf -v REMOTE_TEST_CMD 'test -f %q' "$REMOTE_BINARY_PATH"
    if remote_ssh "$REMOTE_TEST_CMD"; then
        echo "[3/3] Syncing built artifact back..."
        mkdir -p "$(dirname "$LOCAL_BINARY_PATH")"
        "$RSYNC_BIN" -avz -e "$RSYNC_SSH_COMMAND" "$REMOTE:$REMOTE_BINARY_PATH" "$LOCAL_BINARY_PATH"
        echo ""
        echo "=== Remote cargo complete ==="
        ls -la "$LOCAL_BINARY_PATH"
    else
        echo "[3/3] Skipping sync-back: $BINARY_PATH not found on remote"
    fi
else
    echo "[3/3] Skipping binary sync-back"
fi
