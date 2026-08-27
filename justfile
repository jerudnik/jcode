# jcode command source for local iteration and CI-emulation recipes.
# Keep the public recipe names stable: check, test, full-test, package,
# release-check, and lint-docs.

# Throttled check loop - type-checks + tests, skips codegen. Fast feedback.
check: test-python
    python3 -I scripts/check_critical_path_budget.py --expect-digest 249e7ab15579aed123915032972ee0bbdf4d1a787b83e62e148271e1c7a65587 --report target/critical-path-budget.json
    python3 -I scripts/check_guard_nonvacuity.py
    python3 -I scripts/check_test_wiring.py
    scripts/cargo_exec.sh check --locked --workspace --all-targets --all-features

# Every module in tests/, one process each. The glob is the wiring: a new test
# file runs the day it lands, with nothing to remember to add here. Separate
# processes because several of these modules assert on process-global state --
# sys.path scrubbing, cwd -- that a shared runner perturbs, and because a
# module that pollutes sys.path for a later one is a failure mode this repo has
# already paid for. Needs python >= 3.11 for tomllib; CI runs these under
# `nix shell nixpkgs#python3`, not the system interpreter.
test-python:
    #!/usr/bin/env bash
    set -euo pipefail
    for file in tests/test_*.py; do python3 -m unittest "tests.$(basename "$file" .py)"; done

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
    python3 -I scripts/lint_docs.py
    python3 -I scripts/check_docs_references.py
