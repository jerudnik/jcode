# F20b Call Map: Reload-target resolvers, channel/version writers & readers

READ-ONLY investigation of `/Users/jrudnik/labs/jcode`. Goal: collapse self-dev
reload from a multi-channel store to a SINGLE atomic fixed path
`~/.jcode/current/jcode`. Every resolver / writer / reader that touches a
channel or version pointer is mapped below with file:line and classified.

Classification tags used:
- **SELFDEV** = self-dev build/publish/reload loop (F20b's concern)
- **UPDATE-A** = GitHub/source-release update subsystem A (to be deleted in F20c)
- **DAEMON** = shared-server daemon reload/handoff
- **DEBUG** = debug command / selfdev-tool driven
- **TEST** / **EXAMPLE**
- **COSMETIC** = status/doctor display only, not load-bearing for exec

Current on-disk layout verified live:
```
~/.jcode/builds/current/jcode        -> versions/59521d509-dirty-9566decd2f4a/jcode  (symlink)
~/.jcode/builds/shared-server/jcode  -> versions/<label>/jcode
~/.jcode/builds/stable/jcode         -> versions/<label>/jcode
~/.jcode/builds/canary/jcode         -> versions/<label>/jcode
~/.jcode/builds/versions/<label>/jcode   (immutable install target)
markers: builds/{current,stable,shared-server}-version  (text version labels)
```
`~/.jcode/current/jcode` does NOT exist yet. The single fixed path is new.

---

## 1. Reload-target RESOLVERS (client + daemon)

### 1a. Client-side resolvers (all in `crates/jcode-build-support/src/paths.rs`)

**`nix_managed_override_target(externally_managed, is_selfdev_session)`** — paths.rs:573-584
- Pure gate. Returns `Some((launcher_binary_path OR current_exe, "nix-managed"))`
  ONLY when `externally_managed && !is_selfdev_session`; else `None`.
- Wrapper: **`nix_managed_launcher_override(is_selfdev_session)`** — paths.rs:567-569
  (calls `is_externally_managed()` at paths.rs:493 = env `JCODE_NIX_MANAGED` OR
  `running_from_nix_store(current_exe)`).
- Callers: `client_update_candidate` (paths.rs:587), `shared_server_update_candidate`
  (paths.rs:625), `preferred_reload_candidate` (paths.rs:720).

**`client_update_candidate(is_selfdev_session)`** — paths.rs:586-616. Priority:
1. nix override (paths.rs:587) -> launcher/current_exe
2. `current_binary_path()` "current" if exists (paths.rs:591) = `builds/current/jcode`
3. if selfdev: repo `find_dev_binary` (paths.rs:596), then `canary_binary_path` (paths.rs:602)
4. `launcher_binary_path` (paths.rs:607) = `~/.local/bin/jcode`
5. `stable_binary_path` (paths.rs:611) = `builds/stable/jcode`
6. `current_exe` fallback (paths.rs:615)

**`shared_server_update_candidate(is_selfdev_session)`** — paths.rs:624-645. Priority:
1. nix override (paths.rs:625)
2. `shared_server_binary_path` (paths.rs:629) = `builds/shared-server/jcode`
   - selfdev: unconditional (paths.rs:631)
   - non-selfdev: only if `shared_server_channel_is_current_enough()` (paths.rs:635/647-673)
3. `stable_binary_path` (paths.rs:640)
4. `current_exe` (paths.rs:644)

**`preferred_reload_candidate(is_selfdev_session)`** — paths.rs:718-758. Priority:
1. nix override (paths.rs:720)
2. `client_update_candidate(...)` result (paths.rs:724) UNLESS a repo build
   (selfdev: `target/selfdev`|`target/release`; non-selfdev: `target/release`)
   is strictly newer by payload mtime (paths.rs:726-757), in which case the repo
   binary wins.

Resolution order by scenario (client):
- **(i) headed client self-dev** (`is_selfdev=true`, not nix): `preferred_reload_candidate`
  -> newest of {repo target/selfdev|release} vs {`builds/current`, dev, canary,
  launcher, stable, current_exe}. Practically: freshest repo build or `builds/current`.
- **(ii) headed client non-selfdev** (not nix): `builds/current` -> launcher ->
  stable -> current_exe (repo-release only wins if strictly newer).
- **(iii) nix-managed non-selfdev**: launcher (`~/.local/bin/jcode`, itself a symlink
  into `/nix/store`) or current_exe. `builds/` shadow is bypassed entirely.

Callers of the client resolvers (each = an exec/spawn target decision):
- `preferred_reload_candidate`:
  - src/cli/hot_exec.rs:80 — **SELFDEV/reload** `hot_reload`
  - crates/jcode-app-core/src/session_rebuild.rs:218 `publish_rebuild_ready_or_error` — **SELFDEV** rebuild
  - crates/jcode-tui/src/tui/app/handshake.rs:129 `act_on_verdict` — **SELFDEV/identity** re-exec on daemon-mismatch
  - crates/jcode-tui/src/tui/app/tui_lifecycle_runtime.rs:188 — **SELFDEV** background client reload divergence check
  - tests/e2e/binary_integration.rs:718 — **TEST**
- `client_update_candidate`:
  - src/cli/selfdev.rs:136 (stale check) & :160 (target binary) — **SELFDEV**
  - src/cli/hot_exec.rs:158 (post-update exec), :317 (`run_auto_update`), :445 (`reload_server_after_update`) — **UPDATE-A**
  - src/cli/commands/restart.rs:173 `current_restart_restore_exe` — restart command
  - src/cli/startup.rs:285 (post-update restart exec) — **UPDATE-A**
  - crates/jcode-tui/src/tui/app/helpers.rs:166 `launch_client_executable` — client spawn
  - crates/jcode-app-core/src/session_rebuild.rs:76 `rebuild_reload_candidate` — **SELFDEV**
  - crates/jcode-app-core/src/tool/selfdev/mod.rs:666 `launch_binary` — **SELFDEV**
  - crates/jcode-app-core/src/server/jade_relay.rs:1131 — client-spawn (relay)
  - crates/jcode-app-core/src/server/comm_session.rs:139 — client-spawn (headless member)
- `shared_server_update_candidate`:
  - src/cli/dispatch.rs:1196 `spawn_server` — **DAEMON** spawn
  - crates/jcode-app-core/src/server/util.rs:57 `server_update_candidate` (the daemon wrapper) — **DAEMON**

### 1b. Daemon-side resolver (`crates/jcode-app-core/src/server/util.rs`)

The daemon does NOT call `preferred_reload_candidate`. It has its own layered
selection built on `server_update_candidate`:

**`server_update_candidate(is_selfdev)`** — util.rs:56-58 = thin wrapper over
`build::shared_server_update_candidate`.

**`collect_reload_target_candidates(is_selfdev, current_exe)`** — util.rs:343-406.
Builds candidate list:
- "preferred" = `server_update_candidate(is_selfdev)` (exec_candidate=true) util.rs:351
- "alternate" = `server_update_candidate(!is_selfdev)` (exec_candidate=true) util.rs:352
- "channel" `shared-server` = `build::shared_server_binary_path` (exec_candidate=false) util.rs:361
- "channel" `stable` = `build::stable_binary_path` (exec_candidate=false) util.rs:372
- "candidate" `dev` = `get_repo_dir`+`find_dev_binary` (exec_candidate=false) util.rs:383
- "current" `current-exe` (exec_candidate=false) util.rs:395

**`resolve_reload_target(is_selfdev, force)`** — util.rs:110-128. Resolves
current_exe (with `strip_deleted_suffix`, payload resolution, mtime), collects
candidates, then `resolve_reload_target_from_candidates`:
- picks newest of the `exec_candidate=true` set (util.rs:137-142)
- if `force`, may refuse via `forced_stale_shared_server_refusal` (util.rs:149, 494-520)
- applies no-downgrade guard `guarded_reload_target` (util.rs:163-206, 616-647)

**`reload_exec_target(is_selfdev, force)`** — util.rs:88-101. Public entry; logs,
honors refusal, returns `chosen_target()`.

Daemon callers:
- crates/jcode-app-core/src/server/reload.rs:183 — **DAEMON** the actual exec into
  the reload target (`ProcessCommand::new(&binary).arg("serve").arg("--socket")...`)
- crates/jcode-app-core/src/server/client_session.rs:734 `handle_reload` — **DAEMON**
  preflight (refusal + no-update short-circuit via `resolve_reload_target`)
- `server_has_newer_binary()` util.rs:763-821 also scans `server_update_candidate`
  for the "update available" advertisement (util.rs:804). **DAEMON**

**(iii) daemon/shared-server resolution order**: newest across
{`shared-server` candidate (both flavors), stable, dev, current-exe} filtered to
exec_candidates {preferred, alternate}, guarded no-downgrade, with a forced-stale
refusal. In nix-managed non-selfdev it collapses to launcher/current_exe via the
override inside `shared_server_update_candidate`.

---

## 2. Channel/version WRITERS (all defined in `crates/jcode-build-support/src/lib.rs` unless noted)

Low-level primitive: **`update_channel_symlink(channel, version)`** — lib.rs:1114-1133.
Reads `version_binary_path(version)` (must exist), atomically swaps
`builds/<channel>/jcode` symlink via `platform_support::atomic_symlink_swap`
(platform_support.rs:23).

**`update_stable_symlink(version)`** — lib.rs:1136-1140. symlink `stable` + writes `stable-version`.
Callers:
- lib.rs:1382 in `install_local_release` — **UPDATE-A** (local source release)
- crates/jcode-app-core/src/update.rs:342 (`install_main_source_update_blocking`) — **UPDATE-A**
- crates/jcode-app-core/src/update.rs:1102 (release install) — **UPDATE-A**
- server/util.rs:1318,1357,1425,1432,1506 — **TEST**
- tui/app/tests.rs:1426 — **TEST**

**`update_current_symlink(version)`** — lib.rs:1143-1147. symlink `current` + writes `current-version`.
Callers:
- lib.rs:264 in `rollback_pending_activation_for_session` — **SELFDEV** (rollback)
- lib.rs:1184 in `publish_local_current_build_for_source` — **SELFDEV** (main publish)
- lib.rs:1383 in `install_local_release` — **UPDATE-A**
- crates/jcode-app-core/src/update.rs:343, :1103 — **UPDATE-A**
- build-support/tests.rs (many), server/util.rs:1319/1426/1433/1507 — **TEST**

**`update_shared_server_symlink(version)`** — lib.rs:1151-1155. symlink `shared-server` + writes `shared-server-version`.
Callers:
- lib.rs:268 `rollback_pending_activation_for_session` — **SELFDEV**
- lib.rs:1207 `promote_version_to_shared_server` — **SELFDEV/DEBUG** (promote)
- lib.rs:1243 `advance_shared_server_if_tracking_stable` — **UPDATE-A**
- lib.rs:1319 `repair_stale_shared_server_channel` — **DAEMON-repair** (update path)
- lib.rs:1384 `install_local_release` — **UPDATE-A**
- crates/jcode-app-core/src/server/debug_command_exec.rs:634 — **DEBUG** (selfdev reload debug cmd)
- crates/jcode-app-core/src/tool/selfdev/reload.rs:322 — **SELFDEV** (selfdev tool reload)
- server/util.rs:1317/1358/1427/1508, tui/app/tests.rs:1425 — **TEST**

**`update_canary_symlink(hash)`** — lib.rs:1397-1400. symlink `canary` only (no marker file).
Callers:
- crates/jcode-app-core/src/server/debug_command_exec.rs:635 — **DEBUG**

**`update_launcher_symlink(target)`** (private) — paths.rs:512-536. No-ops when
`is_externally_managed()`. Public wrappers:
- **`update_launcher_symlink_to_current()`** — paths.rs:539-542. Callers:
  - lib.rs:265 `rollback_pending_activation_for_session` — **SELFDEV**
  - lib.rs:1185 `publish_local_current_build_for_source` — **SELFDEV**
  - lib.rs:1385 `install_local_release` — **UPDATE-A**
  - crates/jcode-app-core/src/update.rs:344, :1104 — **UPDATE-A**
  - build-support/tests.rs:351,665 — **TEST**
- **`update_launcher_symlink_to_stable()`** — paths.rs:545-548. **No live callers**
  (only re-exports lib.rs:13, build.rs:25). Effectively dead.

**`advance_shared_server_if_tracking_stable(version)`** — lib.rs:1241-1248. Callers:
- crates/jcode-app-core/src/update.rs:336, :1096 — **UPDATE-A**
- server/util.rs:1431, build-support/tests.rs — **TEST**

**`repair_stale_shared_server_channel()`** — lib.rs:1281-1328. Callers:
- src/cli/hot_exec.rs:418 `repair_stale_shared_server_after_update_check` — **UPDATE-A**
- crates/jcode-app-core/src/update.rs:1188 — **UPDATE-A**
- crates/jcode-tui/src/tui/app/remote/server_events.rs:1560 — **DAEMON-repair** (client detects stale server before forced reload)
- build-support/tests.rs — **TEST**

**`promote_version_to_shared_server(version)`** — lib.rs:1205-1209. Callers:
- crates/jcode-build-support/examples/promote_build.rs:7 — **EXAMPLE**

**`install_binary_at_version(source, version)`** — lib.rs:377-380 (atomic
stage->fsync->smoke->rename into `builds/versions/<version>/jcode` via
`copy_binary_to_staging_path` lib.rs:422 + `smoke_test_staged_binary_for_install`
lib.rs:528 + `publish_staged_binary` lib.rs:510). Callers:
- lib.rs:1169 `publish_local_current_build_for_source` — **SELFDEV**
- lib.rs:1381 `install_local_release` — **UPDATE-A**
- lib.rs:1393 `install_version` — **UPDATE-A** (dead-ish helper)
- crates/jcode-app-core/src/update.rs:331, :1092 — **UPDATE-A**
- build-support/tests.rs (many) — **TEST**

**`install_local_release(repo_dir)`** — lib.rs:1373-1388. Installs release, moves
stable+current+shared-server+launcher. Callers:
- src/cli/hot_exec.rs:301 `run_auto_update`, :402 `run_update` — **UPDATE-A**
- crates/jcode-app-core/src/session_rebuild.rs:70, :208 — **SELFDEV** (rebuild path)

**`publish_local_current_build(repo_dir)`** — lib.rs:1199-1202. Callers:
- crates/jcode-build-support/examples/promote_build.rs:6 — **EXAMPLE**

**`publish_local_current_build_for_source(repo_dir, source)`** — lib.rs:1157-1195.
The SELF-DEV publish core (install_binary_at_version -> write sidecar -> validate
-> `update_current_symlink` -> `update_launcher_symlink_to_current`). Callers:
- src/cli/selfdev.rs:155 — **SELFDEV**
- crates/jcode-app-core/src/server/debug_command_exec.rs:632 — **DEBUG**
- crates/jcode-app-core/src/tool/selfdev/reload.rs:289 — **SELFDEV**
- crates/jcode-app-core/src/tool/selfdev/build_queue.rs:367 — **SELFDEV**

Pending-activation set (BuildManifest, lib.rs):
- `set_pending_activation` lib.rs:217 — callers: tool/selfdev/reload.rs:309 (**SELFDEV**), tests
- `clear_pending_activation` lib.rs:222 — no live non-test callers
- `complete_pending_activation_for_session` lib.rs:236 — src/cli/selfdev.rs:230 (**SELFDEV**), tool/selfdev/reload.rs:394 (**SELFDEV**)
- `rollback_pending_activation_for_session` lib.rs:254 (writes `update_current_symlink` + `update_launcher_symlink_to_current` + `update_shared_server_symlink`) — src/cli/selfdev.rs:200, tool/selfdev/reload.rs:324/342/368/411 (**SELFDEV**)
- `reconcile_stale_pending_activation` lib.rs:327 — crates/jcode-app-core/src/server.rs:1186 (**DAEMON** startup reconcile), tests

---

## 3. Channel/version READERS (`storage_helpers.rs` defs)

Marker readers:
- **`read_current_version()`** storage_helpers.rs:111. Live callers: paths.rs:667
  (`shared_server_channel_is_current_enough`), paths.rs:690
  (`version_matches_installed_channel`), lib.rs:362 (reconcile), lib.rs:1168
  (publish previous-current capture), tool/selfdev/status.rs:40 (**COSMETIC**).
- **`read_stable_version()`** storage_helpers.rs:95. Live callers: paths.rs:657/690,
  lib.rs:1230 (`shared_server_tracks_stable`), lib.rs:1282 (`repair_stale...`),
  crates/jcode-tui/src/tui/app/debug_cmds.rs:1029 (`check_stable_version` -> migration, **SELFDEV/migrate**),
  crates/jcode-tui/src/tui/app/construction.rs:5 (`stable_version_if_available`, seeds `known_stable_version`),
  tool/selfdev/status.rs:52 (**COSMETIC**).
- **`read_shared_server_version()`** storage_helpers.rs:127. Live callers: paths.rs:648,
  lib.rs:365 (reconcile), lib.rs:1206 (`promote...` previous), lib.rs:1225
  (`shared_server_tracks_stable`), lib.rs:1298 (`repair...`), tool/selfdev/status.rs:46
  (**COSMETIC**), tool/selfdev/reload.rs:302 (**SELFDEV** capture previous).

Path readers (`builds/<channel>/jcode`):
- **`current_binary_path()`** storage_helpers.rs:33. Callers: paths.rs:540
  (`update_launcher_symlink_to_current`), paths.rs:591 (`client_update_candidate`),
  src/cli/commands/doctor.rs:152 (**COSMETIC**), tool/selfdev/setup.rs:242 (**COSMETIC**).
- **`stable_binary_path()`** storage_helpers.rs:28. Callers: paths.rs:546
  (`update_launcher_symlink_to_stable`, dead), paths.rs:611 & :640 (resolvers),
  lib.rs:1291 (`repair...`), server/util.rs:372 (candidate collection, **DAEMON**),
  crates/jcode-tui/src/tui/ui_status.rs:23 (**COSMETIC** "launched via stable?"),
  crates/jcode-tui/src/tui/app/debug_cmds.rs:1063 (**MIGRATE** — see §4),
  tool/selfdev/setup.rs:246 (**COSMETIC**).
- **`shared_server_binary_path()`** storage_helpers.rs:38. Callers: paths.rs:629
  (resolver), lib.rs:1314 (`repair...`), server/util.rs:361 (candidate, **DAEMON**),
  tool/selfdev/setup.rs:250 (**COSMETIC**).
- **`canary_binary_path()`** storage_helpers.rs:43. Callers: paths.rs:602
  (`client_update_candidate` selfdev branch).
- **`version_binary_path(hash)`** storage_helpers.rs:20. Callers: lib.rs:299
  (`pending_candidate_is_valid`), lib.rs:1119 (`update_channel_symlink`).

Derived predicates:
- **`version_matches_installed_channel(version, git_hash)`** paths.rs:686. **No live
  callers** — only tests (tests.rs:558/562/566) + re-exports. Dead in production.
- **`shared_server_channel_is_current_enough()`** paths.rs:647. Caller: paths.rs:635
  (`shared_server_update_candidate` non-selfdev).
- **`shared_server_tracks_stable()`** lib.rs:1224. Caller: lib.rs:1242
  (`advance_shared_server_if_tracking_stable`) — **UPDATE-A**.
- **`is_release_channel_marker(marker)`** lib.rs:1330. Caller: lib.rs:1306 (`repair...`).
- **`dev_binary_matches_source(binary, source)`** lib.rs:897. Caller: src/cli/selfdev.rs:139
  (stale check) — **SELFDEV**.
- **`current_source_state(repo_dir)`** source_state.rs:91. Callers across selfdev/publish:
  selfdev.rs:135, paths.rs:391 (`run_selfdev_build`), lib.rs:1200, source_state.rs:141/153/216,
  server/debug_command_exec.rs:630, tool/selfdev/mod.rs:741, tool/selfdev/reload.rs:282, examples. **SELFDEV**.
- **`ensure_source_state_matches(repo_dir, expected)`** source_state.rs:140. Callers:
  selfdev.rs:153, paths.rs:402, tool/selfdev/build_queue.rs:339/357. **SELFDEV**.

---

## 4. The migrate escape hatch (`JCODE_MIGRATE_BINARY`)

Full trace:
1. **Detect new stable**: `App::check_stable_version` — debug_cmds.rs:1010-1058.
   Reads `read_stable_version()` (debug_cmds.rs:1029), compares to
   `self.known_stable_version` (app.rs:1129), and at a safe point sets
   `self.pending_migration = Some(current_stable)` (debug_cmds.rs:1053).
   Seeded initial value: `stable_version_if_available()` (construction.rs:5,254,679).
   Polled from the TUI event loop: local.rs:101 (`check_stable_version`) and
   local.rs:103-104 (drives `execute_migration` when `pending_migration.is_some()`).
2. **Set the target**: `App::execute_migration` — debug_cmds.rs:1061-1091.
   - **debug_cmds.rs:1063**: `let stable_binary = crate::build::stable_binary_path()?`
     (must exist). THIS IS THE TARGET-SETTING LINE.
   - **debug_cmds.rs:1083**: `crate::env::set_var("JCODE_MIGRATE_BINARY", stable_binary)`.
   - sets `reload_requested`, `should_quit`.
3. **Read/exec the target**: `hot_reload` — src/cli/hot_exec.rs:59-77.
   - hot_exec.rs:59 reads env, hot_exec.rs:61 checks `binary_path.exists()`,
     hot_exec.rs:63-70 execs it with `--resume <id> --no-update` and
     `env_remove("JCODE_MIGRATE_BINARY")` (hot_exec.rs:67). If missing, warns and
     falls through to `preferred_reload_candidate` (hot_exec.rs:71-77).

To repoint onto the nix-managed binary: change **debug_cmds.rs:1063** to resolve
the nix launcher instead of `stable_binary_path()`. The natural target is
`nix_managed_override_target(...)` result or `launcher_binary_path()` (paths.rs:476).
Note `nix_managed_override_target` is currently private (paths.rs:573); F20b would
need to expose a public accessor (e.g. `nix_managed_reload_target()` or reuse
`launcher_binary_path`). hot_exec.rs:59-77 needs no change (it execs whatever path
the env var holds). This is the ONE reader that must be repointed; everything else
uses the resolvers.

`app.rs:1133` (`pending_migration`) and `app.rs:1129` (`known_stable_version`) are
plain fields; debug_profile.rs:406 only reads `pending_migration` for a debug dump.

---

## 5. What the daemon (`shared-server`) needs

The daemon reload path is entirely mtime/path-driven; it does NOT depend on the
target living under `builds/` or carrying a version label.

- `reload_exec_target` (util.rs:88) / `resolve_reload_target` (util.rs:110) select
  by **payload mtime + no-downgrade guard**, not by directory or label. The chosen
  binary is exec'd at reload.rs:204-207 as `<binary> serve --socket <socket>`.
- The exec/handoff (`prepare_server_exec` reload.rs:16) only manipulates the socket
  and env (`JCODE_READY_FD` etc.). It does not inspect the binary path shape.
- reload_state.rs / reload_recovery.rs carry NO binary-path resolution — grep found
  only test fixtures with `version_label` strings (reload_state.rs:778/809/818),
  which are `RuntimeIdentityProjection` payloads for logging, not resolution inputs.
- `forced_stale_shared_server_refusal` (util.rs:494) and
  `has_strictly_newer_candidate_than_current` (util.rs:247) compare candidate
  mtimes; they reference the label string `"shared-server"` (util.rs:497) purely to
  identify the channel candidate in the list.

So beyond pointing `server_update_candidate`/`shared_server_update_candidate` at
`~/.jcode/current/jcode`, the daemon reload machinery needs NO change to be
label/dir agnostic. BUT: the daemon-side resolver `collect_reload_target_candidates`
(util.rs:343-406) still independently reads `shared_server_binary_path` (util.rs:361)
and `stable_binary_path` (util.rs:372) as non-exec "channel" candidates. Those are
`exec_candidate=false` (advisory only for the no-downgrade newest-mtime scan) but
they participate in `forced_stale_shared_server_refusal` and the newest-mtime pick.
If those channels are collapsed/removed, util.rs:361-382 would reference paths that
may still exist as the single fixed path or vanish. **This is daemon-side and lives
OUTSIDE F20b's owned files (see Ownership gaps).**

---

## 6. Tests that encode channel/version invariants

### `crates/jcode-build-support/src/tests.rs` (43 tests)
REAL invariants to preserve (atomicity / execv-safety / no-downgrade / source-match):
- lib.rs atomic_publish_tests: `concurrent_source_truncation_between_stage_and_rename_preserves_published_copy` (lib.rs:611) — **REAL: atomicity**. `failed_smoke_test_leaves_no_version_entry` (lib.rs:672) — **REAL: smoke gate**.
- tests.rs:88 `dev_binary_matches_source_only_on_exact_metadata` — **REAL: source-match**
- tests.rs:198 `test_binary_version_hash_mismatch_rejects_publish_candidate` — **REAL: source-match**
- tests.rs:216 `test_dev_binary_source_metadata_mismatch_rejects_publish_candidate` — **REAL: source-match**
- tests.rs:161 `installed_immutable_binary_sidecar_projects_exact_runtime_identity` — **REAL: identity** (sidecar semantics, survives)
- tests.rs:105/126 dirty runtime-identity projection — **REAL: identity**
- tests.rs:392 `pending_activation_can_complete_and_roll_back` — **REAL** if pending-activation kept; machinery may be simplified.

Machinery-only (test the multi-channel design being collapsed — likely update/delete):
- tests.rs:307 `test_client_update_candidate_prefers_dev_binary_for_selfdev` — encodes the selfdev fall-through order (will change).
- tests.rs:452 `shared_server_candidate_prefers_approved_channel_over_current`
- tests.rs:474/497/512/531 `normal_shared_server_candidate_*` (repair/allow/ignore stale/corrupt)
- tests.rs:554 `version_match_detects_installed_channel_by_semver_or_git_hash` (tests dead fn)
- tests.rs:574/583/593 `shared_server_tracks_stable_*`
- tests.rs:607/629 `advance_shared_server_*`
- tests.rs:696/744/792 `update_*daemon_reload_target*` / `selfdev_reload_target_diverges*`
- tests.rs:851/881/899/925/949 `repair_*shared_server*`
- tests.rs:1024-1194 `reconcile_*` (8 tests) — **REAL** if pending-activation reconcile kept.
- tests.rs:334/343 launcher-sandbox tests — partially real (launcher no-op under nix).

### `crates/jcode-app-core/src/server/util.rs` tests (29 tests)
- newer_binary_tests (util.rs:862-954): **REAL: no phantom-update / execv-loop guard** (#277/#291). Path strings are illustrative; keep.
- reload_target_tests (util.rs:972-1060): `same_binary_is_always_used`, `newer_candidate_is_used`, `equal_mtime_candidate_is_used`, `strictly_older_candidate_is_blocked_and_uses_current_exe`, `unreadable_candidate_mtime_is_treated_as_downgrade`, `downgrade_without_current_exe_falls_back_to_candidate` — **REAL: no-downgrade + execv-safety**.
- resolve tests (util.rs:1098/1119/1135): forced-stale refusal + non-forced exec — machinery of shared-server pinning; **update** when channels collapse but the forced-stale/no-update semantics are REAL.
- pick_newest tests (util.rs:1190-1258): **REAL: newest-across-flavors** ordering.
- integration-style (util.rs:1302/1343/1411/1496): `selfdev_daemon_reloads_into_fresh_release_after_update`, `selfdev_pin_is_preserved...`, `normal_user_daemon_detects_and_targets_update_after_update`, `freshly_updated_release_daemon_reports_no_phantom_update` — encode multi-channel update-vs-pin behavior; **update/delete** with the collapse but the "no phantom update / no downgrade" property is REAL.
- strip_deleted tests (util.rs:1543-1558): **REAL: in-place-rebuild execv-safety**.

### Other
- crates/jcode-tui/src/tui/app/tests.rs:1425-1426 — pins shared-server old / stable new (migration/repair scenario) — machinery.
- tests/e2e/binary_integration.rs:718 — **REAL: end-to-end reload execs into resolver target**. Keep (works through whatever resolver returns).
- crates/jcode-tui/src/tui/app/handshake.rs tests (:211-284) reference `builds/current/jcode` literals — machinery/identity.

---

## 7. Minimal change set for F20b (route everything through `~/.jcode/current/jcode`, leave subsystem-A writers dead in place)

Guiding idea: introduce ONE fixed path and make the SELF-DEV publish write to it
atomically, then make the resolvers return it FIRST. Do NOT delete the channel
writers/readers (F20c). Keep the atomic primitive.

Owned files: paths.rs, update.rs (build-support has none; note update.rs here is
`crates/jcode-app-core/src/update.rs` which is subsystem A — treat as owned per the
task's `owned_paths` = paths.rs/update.rs/hot_exec.rs/selfdev.rs), lib.rs,
hot_exec.rs, selfdev.rs.

Proposed edits:

1. **paths.rs (new fn)** — add `pub fn current_fixed_binary_path() -> Result<PathBuf>`
   returning `storage::jcode_dir()?.join("current").join(binary_name())`
   (i.e. `~/.jcode/current/jcode`). One-line helper; the single source of truth.

2. **lib.rs `install_binary_at_version` / new publish** — add a sibling
   `publish_current_fixed(source_binary)` that does exactly stage->smoke->rename
   into `~/.jcode/current/` reusing `copy_binary_to_staging_path` (lib.rs:422),
   `smoke_test_staged_binary_for_install` (lib.rs:528), `publish_staged_binary`
   (lib.rs:510). Reuses the guarded atomic primitive verbatim; only the dest dir
   changes from `builds/versions/<label>` to `current/`.

3. **lib.rs `publish_local_current_build_for_source`** (lib.rs:1157) — after (or
   instead of) `update_current_symlink` (lib.rs:1184), also publish into the fixed
   path via the new fn. Minimal: append one call so `~/.jcode/current/jcode` is
   refreshed on every selfdev publish. Leave `update_current_symlink` +
   `update_launcher_symlink_to_current` in place (dead-but-harmless for F20c).

4. **paths.rs `client_update_candidate`** (paths.rs:586) — insert
   `existing_binary(current_fixed_binary_path(), "current-fixed")` as the FIRST
   non-nix candidate (before paths.rs:591 `builds/current`). One inserted block.

5. **paths.rs `shared_server_update_candidate`** (paths.rs:624) — same: prefer the
   fixed path first (after nix override). One inserted block.

6. **paths.rs `preferred_reload_candidate`** (paths.rs:718) — no change needed if it
   keeps delegating to `client_update_candidate` (edit #4 flows through). Verify the
   repo-newer branch (paths.rs:726-757) still compares against the fixed path via
   `resolve_binary_payload`.

7. **selfdev.rs** (selfdev.rs:136,160) — no change required; `client_update_candidate`
   now returns the fixed path first. Optionally assert the published fixed path.

8. **hot_exec.rs** — no change required for the resolver path (execs
   `preferred_reload_candidate`). For the MIGRATE hatch, see §4 (that edit is in
   debug_cmds.rs, OUTSIDE owned files — Ownership gap).

Net effect: all client resolvers return `~/.jcode/current/jcode`, self-dev publish
performs exactly one atomic stage->smoke->rename into it, and the
stable/current/shared-server/canary writers remain in place but unreferenced by the
hot path (subsystem A still writes them; harmless), ready for F20c deletion.

Resolvers living OUTSIDE the four owned files (must be routed too, but are NOT
F20b-owned):
- **crates/jcode-app-core/src/server/util.rs** — `server_update_candidate` (:57),
  `collect_reload_target_candidates` (:343), `reload_exec_target` (:88). The daemon
  reload target. `server_update_candidate` delegates to
  `shared_server_update_candidate` (owned), so editing paths.rs #5 DOES route the
  daemon's preferred/alternate exec-candidates. HOWEVER util.rs:361/372 still add
  `shared_server_binary_path`/`stable_binary_path` as advisory channel candidates
  and the forced-stale refusal keys on `"shared-server"`. Those are non-exec and
  will simply not be "newest" once the fixed path is freshest, so behavior is
  correct WITHOUT editing util.rs — but the dead channel candidates should be
  removed in F20c. Flag for review.
- **crates/jcode-tui/src/tui/app/debug_cmds.rs:1063** — MIGRATE target
  (`stable_binary_path`). Must repoint at nix/fixed path (§4). OUTSIDE owned files.

---

## Ownership gaps (edits required outside paths.rs/update.rs/hot_exec.rs/selfdev.rs)

1. **Daemon reload candidate collection** — `crates/jcode-app-core/src/server/util.rs:343-406`
   (`collect_reload_target_candidates`) reads `shared_server_binary_path` (:361) and
   `stable_binary_path` (:372) directly as advisory candidates, and
   `forced_stale_shared_server_refusal` (:494) / `server_has_newer_binary` (:763)
   scan `server_update_candidate`. Routing works transitively via
   `shared_server_update_candidate` (owned), but the direct `builds/` channel reads
   at util.rs:361/372 remain and should be reconciled. **Raise with the daemon owner.**

2. **Migrate escape hatch target** — `crates/jcode-tui/src/tui/app/debug_cmds.rs:1063`
   sets `JCODE_MIGRATE_BINARY` from `stable_binary_path()`. Must be repointed at the
   nix-managed binary (`launcher_binary_path()` / a new public
   `nix_managed_reload_target()`). Requires exposing a public accessor from paths.rs
   (owned) AND editing debug_cmds.rs (NOT owned). **Raise: needs a debug_cmds.rs edit.**

3. **`nix_managed_override_target` is private** (paths.rs:573). If §4/migrate wants to
   reuse it, F20b must add a `pub` accessor in paths.rs (owned) — fine — but the
   consumer edit lands in debug_cmds.rs (not owned).

4. **selfdev tool / debug publish** — `crates/jcode-app-core/src/tool/selfdev/reload.rs:289,322`,
   `build_queue.rs:367`, `server/debug_command_exec.rs:632,634,635` call
   `publish_local_current_build_for_source` + `update_shared_server_symlink` +
   `update_canary_symlink`. If edit #3 makes `publish_local_current_build_for_source`
   also write the fixed path, these inherit it automatically (good). But their extra
   `update_shared_server_symlink`/`update_canary_symlink` calls are subsystem-A-ish
   writes living in daemon-side files — leave for F20c, flag as not-owned.

---

## Confidence / unknowns

- **High confidence** on the client resolver graph (paths.rs) and the writer/reader
  call sites — all grep-verified with file:line and cross-checked against re-exports
  in `crates/jcode-app-core/src/build.rs` (a pure re-export module, not a caller).
- **High confidence** the daemon reload selection is mtime/path-shape-agnostic:
  reload_state.rs / reload_recovery.rs contain no binary-path resolution (verified by
  grep returning only test fixtures).
- **Medium confidence** on which multi-channel tests must be deleted vs updated: I
  classified by what each asserts, but the exact F20b/F20c split depends on whether
  pending-activation and shared-server pinning survive the collapse. I flagged the
  REAL invariants (atomicity lib.rs:611, smoke-gate lib.rs:672, no-downgrade
  util.rs reload_target_tests, phantom-update util.rs newer_binary_tests,
  deleted-marker strip, source-match) that must be preserved regardless.
- **Unknown / to confirm**: whether `~/.jcode/current/jcode` should be a real file
  (atomic rename target) or itself a symlink into `versions/`. The task says "a
  directory holding one binary, atomically rename-published" -> a real file via
  rename, which the existing `publish_staged_binary` primitive already supports
  unchanged. I assumed a real file, not a symlink.
- **Unknown**: whether the nix launcher (`~/.local/bin/jcode` -> /nix/store) should
  become the migrate target, or a dedicated `~/.jcode/stable`/nix-generation link.
  §4 identifies the single line to change; the exact target is a product decision.
- `install_version` (lib.rs:1391) and `update_launcher_symlink_to_stable`
  (paths.rs:545) and `version_matches_installed_channel` (paths.rs:686) are already
  effectively dead (no live callers) — safe to ignore in F20b, delete in F20c.
