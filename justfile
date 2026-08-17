# jcode command source for local iteration and CI-emulation recipes.
# Keep the public recipe names stable: check, test, full-test, package,
# release-check, and lint-docs.

# Throttled check loop - type-checks + tests, skips codegen. Fast feedback.
check:
    python3 scripts/check_critical_path_budget.py --expect-digest 5ed12e310519a1258ddd0cf9b67674b8aa3198a863f1712771c36b10c5d052ed --report target/critical-path-budget.json
    python3 -m unittest tests.test_guard_nonvacuity
    python3 scripts/check_guard_nonvacuity.py
    scripts/cargo_exec.sh check --locked --workspace --all-targets --all-features

# Fast test gate - compiles the workspace test graph without running it.
test:
    scripts/cargo_exec.sh test --locked --workspace --lib --bins --no-run

# Full CI-equivalent test recipe. ci_local.sh feeds the host target triple in
# JCODE_CI_TARGET so the same recipe can emulate the macOS and Linux cargo
# command lists without reading workflow YAML.
full-test:
    #!/usr/bin/env bash
    set -euo pipefail
    target="${JCODE_CI_TARGET:-$( (rustc -vV 2>/dev/null || true) | sed -n 's/^host: //p' )}"
    scripts/cargo_exec.sh build --locked --release --target "$target"
    "./target/$target/release/jcode" --version
    scripts/cargo_exec.sh test --locked --target "$target" --workspace --lib --bins --no-run
    scripts/cargo_exec.sh test --locked --target "$target" --workspace --lib --bins --exclude jcode-tui --exclude jcode-app-core
    scripts/cargo_exec.sh test --locked --target "$target" -p jcode-tui --lib
    scripts/cargo_exec.sh test --locked --target "$target" -p jcode-app-core --lib
    scripts/cargo_exec.sh test --locked --target "$target" --test provider_matrix --test e2e --no-run
    scripts/cargo_exec.sh test --locked --target "$target" --test provider_matrix
    JCODE_E2E_REQUIRE_BINARY=1 JCODE_E2E_BINARY="$PWD/target/$target/release/jcode" scripts/cargo_exec.sh test --locked --target "$target" --test e2e

# Package sanity check for the root crate. --list verifies the manifest and
# computes the file list without publishing; a full `cargo package` cannot
# work here because the workspace path dependencies are never published to
# crates.io.
package:
    scripts/cargo_exec.sh package --locked -p jcode --allow-dirty --no-verify --list

# Release build + launch smoke.
release-check:
    #!/usr/bin/env bash
    set -euo pipefail
    target="${JCODE_CI_TARGET:-$( (rustc -vV 2>/dev/null || true) | sed -n 's/^host: //p' )}"
    scripts/cargo_exec.sh build --locked --release --target "$target" --bin jcode
    "./target/$target/release/jcode" --version

# Live documentation linting against the repository Vale config.
lint-docs:
    git ls-files -z -- '*.md' ':!scripts/phone-server/**' | xargs -0 vale --config .vale.ini
