# F20b evidence — collapse self-dev reload to a single atomic fixed path

**Node:** F20b (implement / deterministic / W3), depends on F20a. Issue #28.

## What F20b does

Collapses jcode's self-dev reload from a multi-channel version store to ONE
atomic fixed path **`~/.jcode/current/jcode`** (a real file, atomically
rename-published each build). Fast incremental `cargo build` is unchanged. The
existing atomicity guarantee, execv-failure safety, wrapper->payload resolution,
and reload-recovery intents are preserved. The legacy `builds/{stable,current,
shared-server,canary}` channel machinery is left in place, dead, for F20c to
delete.

## Change (all within F20b owned files)

### `crates/jcode-build-support/src/lib.rs`
- Extracted the atomic core of `install_binary_at_version_in_builds_dir` into
  `atomic_publish_binary(source, dest_dir, cleanup_empty_dir)` — the ONE
  stage->fsync->smoke->rename primitive, now shared by the version store and the
  fixed path. `cleanup_empty_dir` distinguishes the per-version dir (removed on
  failure) from the persistent `current/` dir (never removed — it may hold the
  last good binary).
- Added `pub fn publish_current_fixed(source)` → atomically publishes into
  `~/.jcode/current/`.
- `publish_local_current_build_for_source` now calls `publish_current_fixed`
  from the already-smoke-tested versioned copy, so every self-dev publish updates
  the fixed path. The legacy channel writes below it remain (dead) for F20c.

### `crates/jcode-build-support/src/paths.rs`
- `current_fixed_dir()` / `current_fixed_binary_path()` (respect `JCODE_HOME`).
- `nix_managed_fallback_binary()` — the escape-hatch target (nix-managed binary),
  for repointing the migrate hatch off the retired `stable` channel.
- Inserted the fixed path as the FIRST non-nix candidate in BOTH
  `client_update_candidate` (headed clients) AND `shared_server_update_candidate`
  (**the daemon** — its preferred candidate flows through here). This routes
  every process onto the single path; no channel drift, no downgrade, because a
  single atomic path cannot disagree with itself.

### `src/cli/selfdev.rs`
- Low-noise transparent reload summary: `reloaded onto <short_hash>[-dirty]` by
  default; the verbose build command is gated behind `JCODE_SELFDEV_VERBOSE`
  (the `-v` equivalent, since the `SelfDev` command has no verbosity flag; a
  proper `-v` flag is a small follow-up).

## Acceptance gates

### Gate 1 — atomicity: source truncation never yields a truncated exec target
`atomic_publish_tests::fixed_path_publish_survives_source_truncation_between_stage_and_rename`
truncates the source mid-stage (via the after-stage hook) and asserts the
published fixed binary is the complete pre-truncation bytes, that the smoke test
ran the staged temp (not the source, not the final path), and that publish lands
at `<current>/jcode`. The pre-existing version-store truncation test still holds
(both now share `arm_truncation_fixture` + `assert_truncation_preserved`).

### Gate 2 — reload onto the fresh build, no channel drift/downgrade
`fixed_path_resolver_tests::{client_update_candidate,shared_server_candidate}_prefers_fixed_path_over_channels`
publish BOTH a legacy channel and the fixed path, then assert both the client
and the daemon resolvers return `current-fixed` (the fixed path), never the
channel. Because the fixed path is a single atomic file that every resolver
prefers, every reload targets the freshly published binary with no drift and no
downgrade path.

## Evidence

- `test-output.txt` — `cargo test -p jcode-build-support --lib --test-threads=1`:
  63 passed, including the 4 new F20b tests and all pre-existing channel/candidate
  tests (the fixed path coexists with the still-present dead channels).
- All 9 `scripts/preflight.sh` gates pass (ratchets, fmt, clippy). Size discipline:
  the atomic-publish tests were relocated to `atomic_publish_tests.rs` and the
  resolver tests to `fixed_path_resolver_tests.rs` so `lib.rs` (1326) stays under
  its 1403 baseline and `tests.rs` stays at its 1211 baseline — new functionality
  offset by relocating tests into dedicated files, not by bumping budgets.

## Ownership note for F20c
The migrate-hatch target is set at `crates/jcode-tui/src/tui/app/debug_cmds.rs`
(`execute_migration` -> `stable_binary_path()`), which is part of the
stable-channel auto-migrate F20c deletes. `nix_managed_fallback_binary()` is
provided here as its replacement target; the repoint itself belongs to F20c.

## Reproduce
```
nix develop --command cargo test -p jcode-build-support --lib -- --test-threads=1 \
  fixed_path truncation prefers_fixed
```
