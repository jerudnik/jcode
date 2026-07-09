# jcode self-development loop. Run inside the fork devShell: `nix develop`.
# The nix-config operator entrypoint is `just jcode-dev` (cd's here + enters the shell).

# Throttled check loop — type-checks + tests, skips codegen. Fast feedback.
dev-check:
    cargo watch -i "target/*" -x "check --tests"

# Incremental build of the jcode binary on save (what the self-dev re-exec wants).
# `-i "target/*"` is required — without it cargo-watch retriggers on its own
# target/ writes and loops forever.
dev-build:
    cargo watch -i "target/*" -x "build --bin jcode"

# Build with the dedicated selfdev profile (matches jcode's internal self-dev
# build; output lands at target/selfdev/jcode).
dev-selfdev:
    cargo watch -i "target/*" -x "build --bin jcode --profile selfdev"

# One-shot fast check (no watch).
check:
    cargo check --tests
