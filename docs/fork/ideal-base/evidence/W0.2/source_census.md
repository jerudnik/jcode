# W0.2 source census: seam re-audit of WORK_GRAPH.json

Recorded: 2026-07-18 at commit `65175cff4` (branch `main`, dirty worktree per W0.1).
Method: every file/dir/glob named in `owned_paths` of WORK_GRAPH.json was checked
for existence, then each seam's claimed problem was re-confirmed (or refuted)
against current source symbols. No source code was modified.

Verdict summary:

- 25/25 named files exist; 11/11 named directories exist.
- 2 owned-path globs are stale or incomplete (F06, F09). See `gap_nodes.md`.
- 2 nodes have partially pre-existing implementations that must not be
  re-implemented blind (F12 cap primitive, F26 startup sweep). Scope notes below.
- No seam was found to be wholly stale. No node should be removed.

## W1: runtime ownership and persistence

### F01/F02/F03 - shutdown coordinator and activity leases: RETAINED, work real

- `crates/jcode-app-core/src/server/lifecycle.rs` (382 lines) exists:
  - `lifecycle.rs:15` `TemporaryServerPolicy`
  - `lifecycle.rs:90` `persistent_should_exit(client_count, idle_elapsed_secs, idle_timeout_secs)`
  - `lifecycle.rs:159` `spawn_persistent_lifecycle_monitor`
  - `lifecycle.rs:190` `spawn_temporary_lifecycle_monitor`
  - `lifecycle.rs:246` `shutdown_temporary_server`
  - `lifecycle.rs:270` `process_alive`
- `crates/jcode-app-core/src/server.rs` (2242 lines) exists.
- Confirmation the seam is real: `persistent_should_exit` (lifecycle.rs:90-95)
  decides exit from `client_count == 0 && idle_elapsed_secs >= idle_timeout_secs`
  only. Grep for `lease|Lease` across `server.rs` finds no lease abstraction
  (only unrelated `release_retained_heap_if_excessive` at server.rs:1378 and a
  comment at server.rs:1723). There is no work-class census or activity-lease
  authority today. F01/F02/F03 stand.

### F04/F05 - atomic TaskStatusStore: RETAINED, work real

- `crates/jcode-base/src/background.rs` (1390 lines) exists:
  - `background.rs:36` `BackgroundTaskManager`
  - `background.rs:86` `status_path_for`
  - `background.rs:133` `write_status_file` - implemented as
    `let _ = fs::write(path, json).await;` inside
    `if let Ok(json) = serde_json::to_string_pretty(status)`: non-atomic write,
    serialization and IO errors silently swallowed.
  - `background.rs:851` `is_live_task`
  - `background.rs:1385` `global()`
- `crates/jcode-base/src/background/` exists (`model.rs`, `tests.rs`), matching
  F04's `crates/jcode-base/src/background/**` glob.
- Grep for `tempfile|rename|atomic|\.tmp` in background.rs: no atomic
  write-then-rename path exists. F04's gate "no direct non-atomic status-file
  writes remain" targets real current behavior.

### F06 - pooled MCP child ownership and mcp-serve owner-PID liveness: RETAINED, one stale owned path

- `crates/jcode-base/src/mcp/` exists (client.rs, pool.rs, manager.rs, mod.rs,
  protocol.rs, schema_cache.rs, tool.rs plus test files).
  - `client.rs:39` `OwnedChildPermit` (RAII cap permit for one owned MCP child)
  - `client.rs:171` `child: Child` and `client.rs:174` `_child_permit: Option<OwnedChildPermit>`
  - `client.rs:322` `attach_child_permit`
  - `client.rs:374` `shutdown` (kills child at client.rs:383; `start_kill` at client.rs:411)
  - `pool.rs:38` `SharedMcpPool`, `pool.rs:406` `get_shared_pool`
- mcp-serve entry point: `src/cli/mcp_serve.rs` (362 lines),
  `mcp_serve.rs:40` `run_mcp_serve_command`. Grep for
  `owner.pid|owner_pid|OWNER_PID|parent_pid|process_alive` in that file finds
  nothing: mcp-serve has no owner-PID self-liveness today, so the F06 gate is
  real work.
- STALE OWNED PATH: F06 names `src/cli/commands/**/*mcp*`, which matches zero
  files (`find src/cli/commands -iname '*mcp*'` is empty; the directory contains
  doctor.rs, menubar.rs, mobile_server.rs, provider_setup.rs, report_info.rs,
  restart_tests.rs, restart.rs). The mcp-serve command lives at
  `src/cli/mcp_serve.rs` and is dispatched from `src/cli/dispatch.rs` /
  `src/cli/commands.rs`. See gap node GN-1.

### F07/F08 - dead/hung MCP detection and bounded reconnect: RETAINED, work real

- `crates/jcode-base/src/mcp/pool.rs:337` already has a failure cooldown log
  ("Skipping reconnect to '{}' for {}s after recent failure") and a full
  reload/reconnect path at `pool.rs:245`; `manager.rs:432-440` reloads the pool.
- No health deadline or hung-child detection exists: grep for
  `health|evict` across client.rs/pool.rs/manager.rs finds nothing beyond the
  cooldown above. Detecting a killed child before request timeout and a hung
  child by health deadline (F07 gates) is unimplemented. F07/F08 stand, with the
  scope note that a reconnect cooldown primitive already exists at pool.rs:337
  and should be extended, not duplicated.

## W2: recovery reconciliation and resource bounds

### F09 - selfdev pending-activation reconciliation: RETAINED, one incomplete owned-path set

- `crates/jcode-app-core/src/tool/selfdev/` exists (build_queue.rs, launch.rs,
  mod.rs, reload.rs, setup.rs, status.rs, tests.rs).
  - `selfdev/mod.rs:297` `reconcile_pending_state`
  - `selfdev/mod.rs:197` `pending_requests`
- `crates/jcode-build-support/src/lib.rs`:
  - `lib.rs:135` `pending_activation: Option<PendingActivation>` on the manifest
  - `lib.rs:217` `set_pending_activation`, `lib.rs:222` `clear_pending_activation`
  - `lib.rs:236` `complete_pending_activation_for_session`
  - `lib.rs:254` `rollback_pending_activation_for_session`
  - `lib.rs:173` `start_canary` / `lib.rs:156` `binary_for_session` (live canary)
- The type itself is defined outside F09's owned paths:
  `crates/jcode-selfdev-types/src/lib.rs:147` `pub struct PendingActivation`
  with `session_id` (:148) and `requested_at: DateTime<Utc>` (:154). A
  timestamp field already exists; "initiating-session liveness" checks do not
  (completion/rollback at build-support lib.rs:236/254 match `session_id` but
  never test whether that session's process is alive). Work is real. Callers:
  `crates/jcode-app-core/src/build.rs`, `crates/jcode-app-core/src/tool/selfdev/reload.rs`,
  `src/cli/selfdev.rs`. See gap node GN-2 for the owned-path addendum.

### F10/F11 - durable disconnect-cleanup intent: RETAINED, work real

- `crates/jcode-app-core/src/server/client_disconnect_cleanup.rs` exists:
  - `client_disconnect_cleanup.rs:26` `agent_lock_timeout`
  - `client_disconnect_cleanup.rs:80` `skipped_terminal_persistence_on_lock_timeout`
  - `client_disconnect_cleanup.rs:87` `terminal_persistence_incomplete`
  - `client_disconnect_cleanup.rs:122` `cleanup_client_connection`
  - `client_disconnect_cleanup.rs:107` `session_has_live_successor`
- The lock-timeout path today produces a disposition value
  (`skipped_terminal_persistence_on_lock_timeout`) but no durable on-disk
  cleanup record that a restart could reconcile; F10's gate targets that gap.
- `crates/jcode-base/src/session.rs` exists; dead-owner reconciliation with
  persistence-failure ordering at `session.rs:55-67` (calls
  `crate::storage::sweep_stale_pid_markers()` at session.rs:66 only when
  `!persistence_failed`).

### F12/F13 - configurable global caps: RETAINED, partially pre-existing

- A fixed (non-configurable) MCP owned-child cap already exists:
  `crates/jcode-base/src/mcp/client.rs:39` `OwnedChildPermit`,
  `client.rs:43` `try_acquire` against `MAX_OWNED_MCP_CHILDREN`
  (referenced in tests at client.rs:417), `client.rs:52` `current`.
- No cap exists for live background tasks: grep for `MAX|cap|limit` in
  `crates/jcode-base/src/background.rs` finds none.
- `crates/jcode-config-types/src/` exists but holds only `keybindings.rs` and
  `lib.rs`; `McpConfig` lives at `crates/jcode-base/src/mcp/protocol.rs:267`
  (inside F12's `crates/jcode-base/src/mcp/**` glob, so ownership is adequate).
- Scope note: F12 should extend `OwnedChildPermit` into a configurable
  mechanism rather than adding a parallel counter.

### F14 - lifecycle matrix: RETAINED

- `tests/e2e/` exists (14 entries incl. `windows_lifecycle.rs`,
  `reload_multiclient.rs`, `burst_spawn.rs`, `mock_provider.rs`).
- `scripts/**/*lifecycle*` currently matches nothing under `scripts/`
  (only `tests/e2e/windows_lifecycle.rs` matches the name anywhere); the glob is
  a forward reservation for the harness F14 will create, not a stale reference.

## W3: deterministic validation and packaging

### F15/F16/F17 - test hermeticity and blocking rails: RETAINED, work real

- 62 `#[ignore]` attributes exist across `crates`, `src`, `tests` (ripgrep
  count), so the classification census (F15) has a real population.
- `.github/workflows/fork-ci.yml` confirms compile-only/advisory rails:
  - fork-ci.yml:273 and :347 use `--no-run` for workspace lib/bin tests
    (macOS aarch64 and Linux x86_64 respectively).
  - fork-ci.yml:279 and :350 state "jcode-tui stays compile-only on both rails".
  - fork-ci.yml:285 and :356 `--exclude jcode-tui --exclude jcode-app-core`.
  - fork-ci.yml:297 and :367 `--test provider_matrix --test e2e --no-run`.
  - 5 `continue-on-error` occurrences (fork-ci.yml:221, :281, :288, :317).
- `.github/scripts/` exists (`run_with_timeout.py`, `verify_windows_install.ps1`).

### F18 - Nix package build: RETAINED

- `.github/workflows/nix.yml`, `flake.nix`, and `nix/` (`package.nix`,
  `modules/`) all exist.

### F19 - installed mobile assets: RETAINED, work real

- `src/cli/commands/mobile_server.rs:180` `mobile_web_root()` resolves the web
  root by checking `std::env::current_dir()?.join("web/jcode-mobile")` FIRST
  (mobile_server.rs:181-183), then the executable-adjacent path
  (mobile_server.rs:185-190), else bails. The CWD-first ordering is exactly the
  masking hazard F19's gate names ("CWD fallback cannot mask missing packaged
  assets"). `web/jcode-mobile/` exists (9 entries); `scripts/check_web_mobile.sh`
  and `scripts/check_web_mobile_rendered.mjs` exist.

### F20/F21 - installer/updater acquisition: RETAINED

- `tests/test_r10_release_acquisition.py` exists.
- `crates/jcode-app-core/src/update.rs` exists:
  - `update.rs:277` `checksum_asset`, `update.rs:281` `verify_asset_checksum_required`
  - `update.rs:215` `fetch_latest_release_blocking`, `update.rs:268` `platform_asset`
- `scripts/install.sh` exists (plus `install_release.sh`, `uninstall.sh`,
  `test_install_release.sh`).

## W4: security, quality, provenance, hygiene

### F22 - advisory expiry and Homebrew host identity: RETAINED, work real

- `.cargo/audit.toml` exists with 10 `RUSTSEC-*` ignores, each commented, but
  grep for `expiry|expire|until|deadline` in audit.toml and
  `.github/workflows/security.yml` finds no expiring mechanism. The current
  policy relies on a weekly re-run with ignores disabled (audit.toml header
  comment), not per-ignore expiry.
- `.github/workflows/release.yml:436`: the Homebrew publication path exports
  `GIT_SSH_COMMAND="ssh -i ~/.ssh/deploy_key -o StrictHostKeyChecking=no"`
  before `git clone git@github.com:1jehuang/homebrew-jcode.git`
  (release.yml:438). By contrast the AUR path is already strict:
  release.yml:541-545 pins `known_hosts` via `ssh-keyscan` and uses
  `-o StrictHostKeyChecking=yes -o UserKnownHostsFile=...`. F22's gate
  ("no StrictHostKeyChecking=no") targets the exact remaining weak path.
- `docs/SECURITY_DEPENDENCIES.md` and `.github/workflows/security.yml` exist.

### F23 - quality ratchets: RETAINED, partially pre-existing

- `scripts/*budget*` already matches a real budget family:
  `check_code_size_budget.py`, `check_panic_budget.py`,
  `check_startup_budget.sh`, `check_swallowed_error_budget.py`,
  `check_test_size_budget.py`, `check_warning_budget.sh`,
  `check_wildcard_reexport_budget.py`, plus their JSON/txt budget files.
- F23's delta is downward targets and zero-growth trend reporting on top of the
  existing budgets, not creating budgets from scratch.

### F24 - pinned compat inputs, provenance, SBOM: RETAINED, work real

- `scripts/build_linux_compat.sh:22` selects the container as
  `image="${JCODE_COMPAT_IMAGE:-quay.io/pypa/manylinux2014_x86_64}"`, a
  floating tag with no digest pin.
- Grep for `sbom|SBOM|provenance` in `.github/workflows/release.yml` and
  `nix/package.nix` finds nothing: no SBOM/provenance emission exists.
- `crates/jcode-build-meta/` exists (`build.rs`, `Cargo.toml`, `src`).

### F25 - socket/swarm-state hygiene: RETAINED, partially pre-existing

- `crates/jcode-app-core/src/server/socket.rs`:
  - `socket.rs:7` `socket_path`, `socket.rs:16` `debug_socket_path`
  - `socket.rs:41` `cleanup_socket_pair` (removes socket + sibling only)
  - `socket.rs:71` `socket_has_live_listener`, plus the documented stale-socket
    reap rationale below it.
- `crates/jcode-app-core/src/server/swarm_persistence.rs`:
  - `swarm_persistence.rs:197` `control_log_path`,
    `swarm_persistence.rs:204` `current_control_log_len`,
    `swarm_persistence.rs:548` `apply_control_log_tail`. Length is read and
    replayed but never truncated or rotated: no retention bound on the control
    log exists (grep for `truncate|rotate` in retention context is empty),
    matching F25's "terminal logs obey retention" gate.
  - Terminal MEMBER retention already exists:
    `swarm_persistence.rs:376` `terminal_retention` field,
    `swarm_persistence.rs:434` wired from
    `crates/jcode-app-core/src/server/swarm.rs:327`
    `swarm_terminal_member_retention()`
    (`JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS`). F25 must not re-add it.
  - `.bak` corruption-recovery fallback noted at swarm_persistence.rs:442-448;
    grep for `quarantine` under `crates/jcode-app-core/src/server/` finds
    nothing: malformed-state quarantine is unimplemented.
- `crates/jcode-app-core/src/server/lifecycle.rs` also owns
  `metadata_path`/`write_temporary_metadata`/`cleanup_temporary_metadata`
  (lifecycle.rs:98/:106/:151), the sidecar metadata F25's "all sidecars" gate
  refers to.

### F26 - PID marker sweep and telemetry liveness: RETAINED, materially pre-existing startup sweep

- `crates/jcode-storage/src/active_pids.rs`:
  - `active_pids.rs:78` `register_active_pid`, `:89` `unregister_active_pid`
  - `active_pids.rs:362` `sweep_stale_pid_markers` EXISTS and is invoked at
    startup from `crates/jcode-base/src/session.rs:66` (guarded by
    `!persistence_failed`, with an ordering comment at session.rs:62-65).
  - Supporting surface: `active_pids.rs:206` `observe_session_pid_markers`,
    `:227` `remove_session_pid_markers_if_unchanged`, `:320` `PidMarkerSweep`,
    `:339` `remove_active_pid_marker_if_stale_and_matches`,
    `:539` `session_presence`, `:591` `session_counts`.
  - The startup-sweep half of F26's first gate is already implemented; the
    "periodically" half and any liveness gaps remain to verify/implement.
- Telemetry markers are NOT liveness-aware:
  `crates/jcode-telemetry-core/src/state_support.rs:165`
  `prune_active_session_files` prunes purely by file age
  (`max_age = 24h`, state_support.rs:168), with `register_active_session`
  (:192), `observe_active_sessions` (:203), `unregister_active_session` (:209).
  No PID/process check exists. F26's second gate is real.
- The uncompiled duplicate is confirmed:
  `crates/jcode-app-core/src/telemetry_state.rs` is NOT declared as a module
  anywhere in `crates/jcode-app-core/src/lib.rs` (grep `telemetry_state` in
  lib.rs: no match; the compiled copy is `jcode-telemetry-core/src/lib.rs:4
  mod state_support`). The two files DIFFER (diff shows the telemetry-core
  copy carries extra repo-detection: `is_jcode_repo_dir`,
  `find_jcode_repo_in_ancestors`, `telemetry_jcode_repo_dir` at
  state_support.rs:266-309, vs `crate::build::get_repo_dir()` in the app-core
  copy at telemetry_state.rs:279/:286). Equivalence review before removal, as
  F26 requires, is genuinely necessary: they are not textually equivalent.

### F27 - independent hardening verification: RETAINED

- Pure verification node over F22-F26 evidence paths; `docs/fork/ideal-base/reviews/`
  exists. No source seam to audit beyond its inputs above.

## W5: gated externals and signoff (G01-G05, S01-S03)

- All are evidence-only or authorization-gated nodes whose owned paths are
  under `docs/fork/ideal-base/evidence/**` or `reviews/**`; both directories
  exist. No source seam to validate. Retained unchanged.

## Coordinator-owned paths

- `docs/fork/ideal-base/STATE.json` and `docs/fork/ideal-base/DECISIONS.md`
  both exist.
