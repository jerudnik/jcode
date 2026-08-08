#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
strict=0

usage() {
  cat <<'USAGE'
Usage:
  scripts/security_preflight.sh [--strict]

Checks:
  1) Secret scan in tracked source/docs/scripts via gitleaks
  2) World-writable file check under scripts/
  3) Rust dependency advisory scan via cargo-audit (when available)

Options:
  --strict   Fail if cargo-audit is not installed
USAGE
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --strict)
      strict=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
  shift
done

cd "$repo_root"

echo "=== Security Preflight ==="

echo "[1/3] Scanning for likely secrets with gitleaks"
if command -v gitleaks >/dev/null 2>&1; then
  gitleaks dir "$repo_root" --no-banner --redact --config "$repo_root/.gitleaks.toml"
elif command -v nix >/dev/null 2>&1; then
  nix run nixpkgs#gitleaks -- dir "$repo_root" --no-banner --redact --config "$repo_root/.gitleaks.toml"
else
  die "gitleaks is not installed (install it or run inside nix)"
fi


echo "[2/3] Checking script permissions"
if find scripts -type f -perm -0002 -print -quit | grep -q .; then
  find scripts -type f -perm -0002 -print
  die "world-writable files detected under scripts/"
fi

echo "[3/3] Dependency advisories (cargo-audit)"
if command -v cargo >/dev/null 2>&1; then
  cargo audit
elif command -v cargo-audit >/dev/null 2>&1; then
  cargo-audit
elif cargo audit --version >/dev/null 2>&1; then
  cargo audit
else
  if [[ "$strict" -eq 1 ]]; then
    die "cargo-audit is not installed (install with: cargo install cargo-audit --locked)"
  fi
  echo "warning: cargo-audit not installed; skipping advisory check"
fi

echo "=== Security preflight passed ==="
