#!/usr/bin/env bash
# Shared loader for Jcode remote build defaults.
#
# The config file is intentionally a shell fragment so users can write either:
#   JCODE_REMOTE_HOST=builder
# or:
#   export JCODE_REMOTE_HOST=builder
#
# Explicit environment variables take precedence over values loaded from the
# config file. This lets callers temporarily disable remote builds with, for
# example, `JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh check`.

jcode_remote_config_path() {
  if [[ -n "${JCODE_REMOTE_CONFIG:-}" ]]; then
    printf '%s\n' "$JCODE_REMOTE_CONFIG"
  elif [[ -n "${XDG_CONFIG_HOME:-}" ]]; then
    printf '%s\n' "$XDG_CONFIG_HOME/jcode/remote-build.env"
  elif [[ -n "${HOME:-}" ]]; then
    printf '%s\n' "$HOME/.config/jcode/remote-build.env"
  fi
}

jcode_load_remote_config() {
  local config_file
  config_file="$(jcode_remote_config_path)"
  [[ -n "$config_file" && -f "$config_file" ]] || return 0

  local supported=(
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
  local preserved_names=()
  local preserved_values=()
  local name
  for name in "${supported[@]}"; do
    if [[ ${!name+x} ]]; then
      preserved_names+=("$name")
      preserved_values+=("${!name}")
    fi
  done

  # shellcheck source=/dev/null
  source "$config_file"

  local i
  for ((i = 0; i < ${#preserved_names[@]}; i++)); do
    printf -v "${preserved_names[$i]}" '%s' "${preserved_values[$i]}"
  done
}
