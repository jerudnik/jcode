#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cargo_exec="$repo_root/scripts/cargo_exec.sh"

run_cargo() {
  (cd "$repo_root" && "$cargo_exec" "$@")
}

echo "=== Full test flow ==="

echo "=== 1/7 Fast suite (nextest: lib + bins, JUnit + slow-test artifacts) ==="
"$repo_root/scripts/test_fast.sh" "$@"

echo "=== 2/7 Doctests ==="
run_cargo test --workspace --doc "$@"

echo "=== 3/7 Process-sensitive binary integration suite ==="
run_cargo test -p jcode --test binary_integration -- --nocapture --test-threads=1

echo "=== 4/7 Desktop coverage suite (explicitly ignored) ==="
if [[ "${OSTYPE:-}" == darwin* ]]; then
  run_cargo test -p jcode-app-core tool::computer::coverage -- --ignored --nocapture --test-threads=1
else
  echo "Skipping desktop coverage suite: macOS only."
fi

echo "=== 5/7 Info-widget stability benchmarks (explicitly ignored) ==="
run_cargo test -p jcode-tui --lib info_widget_stability::tests::demo_ -- --ignored --nocapture

echo "=== 6/7 Session-picker benchmarks (explicitly ignored) ==="
run_cargo test -p jcode-tui --lib --release benchmark_resume_op -- --ignored --nocapture

echo "=== 7/7 Manual registry report (explicitly ignored) ==="
run_cargo test -p jcode-app-core --lib tool::tests::print_tool_definition_token_report -- --ignored --nocapture

if [[ -x "$repo_root/target/release/jcode" ]]; then
  echo "=== Optional explicit self-dev reload stressor (ignored) ==="
  run_cargo test -p jcode --test binary_integration binary_integration_selfdev_full_reload_resumes_session_quickly -- --ignored --nocapture
else
  echo "Skipping explicit self-dev reload stressor: build target/release/jcode first."
fi

if [[ "${JCODE_REAL_PROVIDER:-0}" == "1" ]]; then
  echo "=== Optional credential-gated binary integration tests (ignored) ==="
  run_cargo test -p jcode --test binary_integration \
    binary_integration_independent_claude \
    binary_integration_openai_provider \
    -- --ignored --nocapture
else
  echo "Skipping credential-gated binary integration tests: set JCODE_REAL_PROVIDER=1 to run them."
fi

echo ""
echo "Full test flow complete."
