# Implementation log

## 2026-07-10 — Plan reframe

- Commit: `0d242297e docs(audit): reframe credential and parser fixes`
- Replaced WI-1 and WI-4 with the final real-cut designs.
- Preserved superseded rationale with links to `.audit/SOL-ASSESSMENT-R3.md` holes #1 and #2.

## 2026-07-10 — Baseline: configurable memory embedding credential env

- Files: `crates/jcode-base/src/config.rs`, `config/default_file.rs`, `config/env_overrides.rs`, `embedding_backend.rs`, `crates/jcode-config-types/src/lib.rs`.
- Added `agents.memory_embedding_api_key_env`, its env override/default-file documentation, cache fingerprint input, and remote embedding credential selection.
- Validation:
  - `nix develop --command cargo test -p jcode-base --lib config_env_fingerprint_tracks_every_apply_env_override_var` -> exit 0, 1 passed.
  - `nix develop --command cargo test -p jcode-base --lib embedding_backend` -> exit 0, 3 passed.
  - `nix develop --command cargo check -p jcode-base` -> exit 0.
- Only pre-existing dead-code/test warnings were emitted.
