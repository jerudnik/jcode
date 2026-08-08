#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cargo_exec="$repo_root/scripts/cargo_exec.sh"
junit_xml="$repo_root/target/nextest/ci/junit.xml"
coverage_dir="$repo_root/target/llvm-cov"

run_cargo() {
  (cd "$repo_root" && "$cargo_exec" "$@")
}

if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
  nix_bin=${NIX_BIN:-}
  if [[ -z "$nix_bin" ]]; then
    if command -v nix >/dev/null 2>&1; then
      nix_bin=$(command -v nix)
    elif [[ -x /nix/var/nix/profiles/default/bin/nix ]]; then
      nix_bin=/nix/var/nix/profiles/default/bin/nix
    elif [[ -x "$HOME/.nix-profile/bin/nix" ]]; then
      nix_bin="$HOME/.nix-profile/bin/nix"
    elif [[ -x /run/current-system/sw/bin/nix ]]; then
      nix_bin=/run/current-system/sw/bin/nix
    fi
  fi
  if [[ -z "$nix_bin" ]]; then
    echo "coverage.sh needs cargo-llvm-cov or nix to provide it." >&2
    exit 127
  fi

  exec "$nix_bin" shell nixpkgs#cargo nixpkgs#cargo-nextest nixpkgs#cargo-llvm-cov nixpkgs#llvmPackages_21.llvm --command "$repo_root/scripts/coverage.sh" "$@"
fi

mkdir -p "$coverage_dir"

if [[ -z "${LLVM_COV:-}" ]]; then
  LLVM_COV=$(command -v llvm-cov)
  export LLVM_COV
fi

if [[ -z "${LLVM_PROFDATA:-}" ]]; then
  LLVM_PROFDATA=$(command -v llvm-profdata)
  export LLVM_PROFDATA
fi

echo "=== Coverage pass 1/3: workspace tests through nextest ==="
run_cargo llvm-cov --no-report nextest --profile ci --workspace --lib --bins --tests --status-level slow --final-status-level slow

echo ""
echo "=== Coverage pass 2/3: doctests ==="
run_cargo llvm-cov --no-report --workspace --doc

echo ""
echo "=== Coverage pass 3/3: merged lcov report ==="
run_cargo llvm-cov report --doctests --lcov --output-path "$coverage_dir/lcov.info"

echo "JUnit report: ${junit_xml#$repo_root/}"
echo "Coverage artifact: $coverage_dir/lcov.info"
