# Investigator B: Can subsystem B (source-build + channel management) be collapsed to a single fixed reload path?

Investigation of `/Users/jrudnik/labs/jcode` (READ-ONLY). Evidence cited as file:line.

Status: COMPLETE. (Full Verdict at the END of this file; Q1-Q5 in between.)

---

## Q1. Exact reload path resolution

### `src/cli/hot_exec.rs`

**`hot_restart`** (hot_exec.rs:35-52): Does NOT use channels at all. Exec's `std::env::current_exe()` (the running payload) with `--resume <session>`. Adds `self-dev` arg if `client_selfdev_requested()`. On exec failure, returns error (current process keeps running because execv only replaces on success).

**`hot_reload`** (hot_exec.rs:54-130):
1. First checks `JCODE_MIGRATE_BINARY` env var (hot_exec.rs:59-77). If set and the path exists, exec's THAT path with `--resume ... --no-update` and removes the env var. This is the "migrate to stable" path.
2. Otherwise resolves via `build::preferred_reload_candidate(is_selfdev)` (hot_exec.rs:80). If `None`, errors "No reloadable binary found".
3. Retries exec up to 3 times on ENOENT with 200ms sleep (hot_exec.rs:102-125). This handles a race where a channel symlink is mid-swap.

**`hot_update`** (hot_exec.rs:132-211): GitHub-release path (subsystem A). Calls `update::check_for_update_blocking()`, downloads via `download_and_install_blocking_with_progress`, then exec's `build::client_update_candidate(is_selfdev)` (falls back to downloaded path). On "up to date", calls `repair_stale_shared_server_after_update_check()` then falls through to re-exec `current_exe()` with `--no-update`.

### Resolution priority (crates/jcode-build-support/src/paths.rs)

**`preferred_reload_candidate(is_selfdev)`** (paths.rs:696-736):
1. `nix_managed_launcher_override(is_selfdev)` -> if Some, RETURN (paths.rs:698). [DORMANT: gated on JCODE_NIX_MANAGED]
2. `candidate = client_update_candidate(is_selfdev)` (paths.rs:702)
3. `repo_binary` = newest of repo `target/selfdev` + `target/release` (selfdev) or just `target/release` (non-selfdev) (paths.rs:704-713)
4. If repo_binary is newer (by payload mtime) than candidate -> use repo_binary, else candidate (paths.rs:728-735)

**`client_update_candidate(is_selfdev)`** (paths.rs:564-594) priority:
1. `nix_managed_launcher_override` -> RETURN if Some [DORMANT]
2. `current` channel: `~/.jcode/builds/current/jcode` if exists (paths.rs:569)
3. (selfdev only) repo `find_dev_binary` -> newest of target/selfdev, target/release (paths.rs:573-578)
4. (selfdev only) `canary` channel `~/.jcode/builds/canary/jcode` (paths.rs:580)
5. `launcher` `~/.local/bin/jcode` (paths.rs:585)
6. `stable` channel `~/.jcode/builds/stable/jcode` (paths.rs:589)
7. `std::env::current_exe()` (paths.rs:593)

**`shared_server_update_candidate(is_selfdev)`** (paths.rs:602-623) priority — used by the DAEMON:
1. `nix_managed_launcher_override` -> RETURN if Some [DORMANT]
2. `shared-server` channel `~/.jcode/builds/shared-server/jcode`:
   - selfdev: use if exists (paths.rs:608-611)
   - non-selfdev: use only if `shared_server_channel_is_current_enough()` (marker == stable or == current) (paths.rs:612-616)
3. `stable` channel (paths.rs:618)
4. `current_exe()` (paths.rs:622)
   NOTE: deliberately does NOT follow fast-moving `current` channel (comment paths.rs:596-601).

**`nix_managed_override_target(externally_managed, is_selfdev)`** (paths.rs:551-562):
- Only fires when `externally_managed && !is_selfdev`.
- Returns launcher `~/.local/bin/jcode` if it exists, else `current_exe()`, labeled "nix-managed".
- This ALREADY bypasses the entire builds/ shadow (current/canary/shared-server/stable).

**Channel binary paths** (storage_helpers.rs):
- `current_binary_path` = `~/.jcode/builds/current/jcode` (:33)
- `stable_binary_path` = `~/.jcode/builds/stable/jcode` (:28)
- `shared_server_binary_path` = `~/.jcode/builds/shared-server/jcode` (:38)
- `canary_binary_path` = `~/.jcode/builds/canary/jcode` (:43)
- `version_binary_path(v)` = `~/.jcode/builds/versions/<v>/jcode` (:20)
- `launcher_binary_path` = `~/.local/bin/jcode` (paths.rs:476, launcher_dir paths.rs:449)

Channel dirs are symlinks: `builds/<channel>/jcode` -> `builds/versions/<v>/jcode` (via `update_channel_symlink`, lib.rs:1114-1133).


## Q2. Is there real rollback?

YES — but ONLY in the self-dev canary path (subsystem B), and it restores CHANNEL SYMLINKS, not the running process. There is NO automatic re-exec of a previous binary on crash.

### The canary/pending-activation rollback machinery
- `PendingActivation` (jcode-selfdev-types/src/lib.rs:147) records `previous_current_version` + `previous_shared_server_version` + `session_id` + `new_version`.
- Set on `/reload` (self-dev): `tool/selfdev/reload.rs:309-319` writes pending activation capturing the previous channel versions, then `update_shared_server_symlink(&hash)` (reload.rs:322).
- **Rollback = restore channel symlinks** (`rollback_pending_activation_for_session`, lib.rs:254-274):
  ```
  if let Some(previous) = pending.previous_current_version.as_deref() {
      update_current_symlink(previous)?;
      update_launcher_symlink_to_current()?;
  }
  if let Some(previous) = pending.previous_shared_server_version.as_deref() {
      update_shared_server_symlink(previous)?;
  }
  manifest.canary_status = Some(CanaryStatus::Failed);
  ```
  It re-points `builds/current` and `builds/shared-server` symlinks back to the prior `versions/<label>/` entry. It does NOT exec anything.

### Where rollback fires
- reload.rs:324,342,368,401,411 — rollback on: shared-server symlink update failure, reload-ctx save failure, ack timeout, replacement server `Failed`/unconfirmed readiness.
- src/cli/selfdev.rs:199 — on resume, if `ReloadPhase::Failed` marker seen, roll back pending activation.
- server.rs:1186 — `reconcile_stale_pending_activation` at startup for a dead initiator (lib.rs:327-374): validates candidate binary's `.source.json`, either completes or rolls back symlinks.

### The "safety" for the running process is execv-failure semantics, NOT rollback
- In `hot_reload` (hot_exec.rs:114) and server `await_reload_signal` (reload.rs:207), a failed `replace_process` (execv) leaves the CURRENT process image running because execv only replaces on success. The server then re-enters termination (reload.rs:241 `reload_exec_failed`) -> exits code 42; the client observes `ReloadPhase::Failed` and rolls back the SYMLINKS.
- The downgrade guard in `guarded_reload_target` (util.rs:616-647) is the other "safety": if the chosen candidate is strictly OLDER by payload mtime than the running exe, it re-execs the CURRENT exe instead (util.rs:640-644), or falls back to candidate if current exe was unlinked (util.rs:182-188).
- `forced_stale_shared_server_refusal` (util.rs:494-520) refuses a forced reload when shared-server is stale relative to a strictly-newer candidate.

### Key point for the maintainer
The rollback does NOT protect against "new binary crashes at runtime after successfully exec'ing and binding the socket." Once the replacement server becomes Ready, `complete_pending_activation_for_session` is called (reload.rs:394) and the pending record is cleared. A crash-loop *after* readiness is not auto-reverted by this machinery. The rollback only covers "replacement never became ready" (exec failure, early crash before socket-ready, ack timeout). It relies on the OLD binary's version still being installed in `~/.jcode/builds/versions/` to restore the symlink.


## Q3. Multi-process coordination

### Process topology (what the code does)
- **Shared daemon ("shared-server")**: long-lived `jcode serve --socket ...` process. Reloads IN PLACE via execv in `await_reload_signal` (reload.rs:57-244), choosing target via `reload_exec_target` -> `resolve_reload_target` -> `collect_reload_target_candidates` which uses `server_update_candidate` = `build::shared_server_update_candidate` (util.rs:56-58, 351-352).
- **Headless swarm agents**: run IN-PROCESS inside the daemon (comm_session.rs:683 "Inline workers run in-process like headless ones"; `create_headless_session`). They do NOT have their own binary; they ride the daemon's execv. So after a daemon reload they are on the daemon's new binary automatically (they are re-spawned from reload-recovery intents, reload.rs:246-367 / reload_recovery.rs).
- **Headed/visible sessions**: spawned as separate terminal processes via `spawn_visible_session_window_with_context` -> `client_update_candidate` (comm_session.rs:139) and `jade_relay.rs:1131`. Each TUI client reloads itself via `preferred_reload_candidate` (hot_exec.rs:80 / session_rebuild.rs:218).

### Role of the shared-server channel + version labels
The `shared-server` channel is a DELIBERATELY SLOWER-MOVING pointer than `current`:
- `shared_server_update_candidate` (paths.rs:602-623) intentionally does NOT follow the fast-moving `current` channel (comment paths.rs:596-601): "local dirty self-dev builds stop taking out every client by accident."
- For a NON-selfdev daemon it only accepts `shared-server` when `shared_server_channel_is_current_enough()` (marker == stable or == current) (paths.rs:612-616, 625-651). Otherwise it falls to `stable`.
- Version labels (`current-version`, `stable-version`, `shared-server-version` marker files) let processes AGREE on "are we the same build?" WITHOUT comparing mtimes. Used by `shared_server_channel_is_current_enough`, `shared_server_tracks_stable`, `repair_stale_shared_server_channel`, `version_matches_installed_channel`.

### What coordination would be LOST if every process exec'd one fixed path
The channel machinery solves TWO real problems in the *self-managed* (non-nix) model:
1. **Blast-radius isolation**: a client's local dirty self-dev build lands on `current` immediately, but the daemon (serving ALL clients) stays on `shared-server`/`stable` so one person's broken build does not take down every client sharing the daemon. With a single fixed path, a dirty self-dev build would immediately become the daemon's reload target too. HOWEVER: in the nix-native model the reload target is the nix `result` (an atomic, tested, whole-system build), so a "dirty local build taking out the daemon" cannot happen the same way. The self-dev loop is a separate concern (see Open Questions).
2. **Version drift / phantom-update loops**: `shared_server_channel_is_current_enough` + `server_has_newer_binary` (util.rs:763-821) + payload resolution exist to stop a daemon and a client from disagreeing about "is there an update" and wedging into an infinite reload loop (issues #277, #291, and the "self-certifying-channel version-drift incident" the comments reference). This drift is INTRINSIC to having multiple independently-moving pointers (current vs stable vs shared-server vs launcher). A single fixed path (`result`) that nix swaps atomically ELIMINATES the class of bug: all processes readlink the same `result` -> same store path, so they cannot disagree.

### Is the "self-certifying-channel version-drift incident" relevant?
Yes, directly. It is cited at paths.rs:542 and 771 and motivates `nix_managed_launcher_override`. The incident: a self-managed channel that "certifies itself" as current could drift so the running daemon serves a stale binary while advertising it is current. `nix_managed_launcher_override` was ADDED to make nix-managed mode bypass the entire `builds/` shadow precisely to avoid re-introducing that drift. In other words: the maintainers ALREADY concluded that under nix, the single-fixed-path (launcher/result) model is the correct one and the channel shadow is the hazard.

### `advance_shared_server_if_tracking_stable` / `repair_stale_shared_server_channel` / `shared_server_tracks_stable` (lib.rs:1224-1369)
These exist to drag a long-lived daemon's `shared-server` pointer forward when `/update` (subsystem A, GitHub release) moved `stable` but the daemon never re-ran the install path. They are entirely about reconciling the multiple self-managed pointers. Under a single fixed nix path there is nothing to advance/repair: nix rebuild swaps `result`, and `server reload --force` re-execs it.


## Q4. Nix-managed hooks present but dormant

### `JCODE_NIX_MANAGED` is NOT set anywhere in packaging (confirmed)
Full grep across the repo for `JCODE_NIX_MANAGED`:
- `crates/jcode-build-support/src/paths.rs:487` — the READER (`is_externally_managed()`).
- `crates/jcode-build-support/src/paths.rs:550` — doc comment.
- `docs/NIX.md:202` — docs describing intended behavior.
- `docs/fork-sync-policy.md:63` — docs.
- `src/cli/commands/doctor.rs:474` — another READER (`running_vs_installed_drift`).

Crucially, `nix/package.nix`, `nix/modules/home-manager.nix`, and `flake.nix` NEVER set it. `home-manager.nix` only sets `JCODE_HOME` (:121-122). `nix/package.nix` sets `JCODE_BUILD_*` build-meta and does NOT wrap the binary with any runtime env. There is no `makeWrapper`/`wrapProgram` exporting `JCODE_NIX_MANAGED`.

**Conclusion: `is_externally_managed()` returns false in the real nix install. The entire `nix_managed_launcher_override` path is DEAD CODE today.** The only test coverage is `nix_managed_override_only_for_managed_non_selfdev` (paths.rs:766-779) which calls the pure `nix_managed_override_target` directly.

### What activates if the nix package DID set `JCODE_NIX_MANAGED`
`nix_managed_override_target(externally_managed=true, is_selfdev=false)` (paths.rs:551-562) returns the launcher `~/.local/bin/jcode` if it exists, else `current_exe()`, labeled "nix-managed". This override is consulted FIRST in all three resolvers:
- `client_update_candidate` (paths.rs:565)
- `shared_server_update_candidate` (paths.rs:603)
- `preferred_reload_candidate` (paths.rs:698)

So with the flag set, non-selfdev reloads/updates/spawns ALL bypass `builds/{current,canary,shared-server,stable,versions}` and resolve straight to the launcher (the nix profile binary). Additionally `update_launcher_symlink` becomes a no-op (paths.rs:495) so jcode stops rewriting `~/.local/bin/jcode`.

**Yes — the existing `nix_managed_override_target` logic already implements most of the "single fixed path" model.** It reduces resolution to "the launcher, or the running exe." The one gap: it points at the LAUNCHER (`~/.local/bin/jcode`), not the nix `result` symlink or store path directly. In a home-manager install the launcher is `~/.nix-profile/bin/jcode` (a store symlink), which IS effectively the fixed path. But note: self-dev sessions (`is_selfdev=true`) STILL opt out of the override (paths.rs:555, 571) and fall through to `builds/`+repo resolution. So even in nix-managed mode the self-dev loop keeps using `current`/`canary`/repo builds unless that path is also collapsed.

Also note: setting `JCODE_NIX_MANAGED` only *disables* auto-update (`is_release_build`/pull machinery) and *disables* launcher rewriting; it does not by itself DELETE the version store or channel code — the code still exists and is still used by every self-dev session.


## Q5. Concrete deletion impact (call sites)

Legend: **[CRIT]** correctness-critical (removing changes runtime behavior/reload correctness), **[COSMETIC]** safe (status/debug/docs/tests), **[A]** belongs to subsystem A (GitHub self-update, dead for this hard fork).

### `install_binary_at_version` (the version store)
- lib.rs:1169 (`publish_local_current_build_for_source`) **[CRIT — self-dev publish path]**
- lib.rs:1381 (`install_local_release`) **[CRIT — used by `/rebuild`, `run_update` source path]**
- lib.rs:1393 (`install_version`) — only referenced by build.rs re-export; effectively dead **[COSMETIC]**
- update.rs:328, update.rs:1092 (source/GitHub update install) **[A]**

### `install_local_release`
- hot_exec.rs:301 (`run_auto_update`), hot_exec.rs:389 (`run_update` non-release path) **[CRIT for source-build /update; but this is the subsystem being replaced]**
- session_rebuild.rs:70, 208 (`/rebuild` background + foreground) **[CRIT — maintainer's `/rebuild`]**

### `publish_local_current_build_for_source` (self-dev publish -> current channel)
- selfdev.rs:155 (`jcode self-dev` launch, after build) **[CRIT — self-dev loop]**
- debug_command_exec.rs:632 (debug publish command) **[COSMETIC/debug]**
- tool/selfdev/build_queue.rs:367 **[CRIT — self-dev build queue]**
- tool/selfdev/reload.rs:289 (`/reload` self-dev) **[CRIT — the daily hot-reload]**

### `update_stable_symlink` / `update_current_symlink`
- lib.rs:1382-1383 (`install_local_release`), lib.rs:1184 (`publish_local_current_build_for_source`), lib.rs:264 (rollback) **[CRIT if channels kept]**
- update.rs:339-340, 1102-1103 **[A]**

### `update_shared_server_symlink`
- lib.rs:1207 (`promote_version_to_shared_server`), 1243 (`advance_shared_server_if_tracking_stable`), 1319 (`repair_stale_shared_server_channel`), 1384 (`install_local_release`), 268 (rollback) **[CRIT — daemon reload target selection]**
- debug_command_exec.rs:634 (debug) **[COSMETIC/debug]**
- tool/selfdev/reload.rs:322 (`/reload` promotes new build onto shared-server) **[CRIT — this is how a self-dev reload becomes the daemon's target]**

### `update_canary_symlink`
- debug_command_exec.rs:635 only **[COSMETIC/debug]** (no production caller)

### `promote_version_to_shared_server`
- examples/promote_build.rs:7 only **[COSMETIC — example binary]**

### `advance_shared_server_if_tracking_stable`
- update.rs:333, 1096 **[A — only the GitHub/source update path calls it]**

### `repair_stale_shared_server_channel`
- hot_exec.rs:405 (`repair_stale_shared_server_after_update_check`) **[A — only reached from update commands]**
- server_events.rs:1560 (TUI on a "stale server" event) **[CRIT-ish — cross-process staleness self-heal, but only matters because multiple pointers exist]**
- update.rs:1188 **[A]**

### `shared_server_tracks_stable`
- lib.rs:1242 only (internal to `advance_shared_server_if_tracking_stable`) **[A-adjacent]**

### Channel path getters (used by RESOLVERS — these are the load-bearing ones)
- `current_binary_path`: paths.rs:518,569 (resolvers), doctor.rs:152, debug_testers.rs:115, tool/selfdev/setup.rs:242 **[CRIT in resolvers; COSMETIC elsewhere]**
- `stable_binary_path`: paths.rs:524,589,618 (resolvers), lib.rs:1291 (repair), util.rs:372 (candidate collection), debug_cmds.rs:1063 (`JCODE_MIGRATE_BINARY` migration), ui_status.rs:23, setup.rs:246 **[CRIT in resolvers + migration; COSMETIC in status]**
- `shared_server_binary_path`: paths.rs:607 (resolver), util.rs:361 (candidate collection), lib.rs:1314 (repair), setup.rs:250 **[CRIT in resolver/candidate collection]**
- `canary_binary_path`: paths.rs:580 (resolver, selfdev-only), debug_testers.rs:118,127 **[CRIT in selfdev resolver; else COSMETIC]**
- `version_binary_path`: lib.rs:299 (pending validation), 1119 (`update_channel_symlink`) **[CRIT if version store kept]**

### Reload-target selection consumers that must be repointed at the fixed path
- `preferred_reload_candidate`: hot_exec.rs:80, session_rebuild.rs:218 **[CRIT — client reload]**
- `client_update_candidate`: hot_exec.rs:158,317,432; startup.rs:285; restart.rs:173; selfdev.rs:136,160; session_rebuild.rs:76; comm_session.rs:139; jade_relay.rs:1131; tool/selfdev/mod.rs:666 **[CRIT — spawn/update/visible-window binary]**
- `shared_server_update_candidate`: dispatch.rs:1196 (spawn_server), util.rs:57 (server reload) **[CRIT — daemon binary]**

**Net:** If you collapse to one fixed path, the correct approach is NOT to delete each getter individually but to make `preferred_reload_candidate`, `client_update_candidate`, and `shared_server_update_candidate` (and server `reload_exec_target`/`collect_reload_target_candidates`) all return the SAME fixed path. That is exactly what `nix_managed_launcher_override` already does (Q4) — flipping `JCODE_NIX_MANAGED` on collapses the three resolvers for the NON-selfdev case in one move. The self-dev case (`is_selfdev=true`) is the remaining branch that still needs `current`/`canary`/repo builds.


## Must-preserve (correctness-critical)

1. **Atomic swap of the reload target.** The version store's real value is `install_binary_at_version_in_builds_dir` (lib.rs:382-420) + `copy_binary_to_staging_path` (create_new temp, fsync, rename, zero-byte guard) + atomic symlink swap (update_channel_symlink lib.rs:1114-1133). Multiple long-lived processes execv this path; if it can ever be observed half-written, a reload can exec a truncated ELF. A test explicitly guards this: `concurrent_source_truncation_between_stage_and_rename_preserves_published_copy` (lib.rs:611). Under nix, the `result` store path IS atomic (nix swaps the symlink), so this invariant is preserved for free. **If you instead point reloads at `target/selfdev/jcode` directly, you LOSE atomicity: cargo writes that path in place during a build, and a concurrent daemon reload could exec a partial binary.** This is the single biggest correctness risk in the collapse.

2. **No-downgrade guard on the daemon.** `guarded_reload_target` (util.rs:616-647) + `forced_stale_shared_server_refusal` (util.rs:494-520) prevent a forced reload from exec'ing an older binary and downgrading every client. This is independent of the channel store (it compares payload mtimes of whatever candidates exist) and should be kept even with a single path, OR made moot by the fact that nix `result` only ever moves forward. Keep the guard or prove the fixed path is monotonic.

3. **execv-failure leaves current process alive.** hot_exec.rs:114-124, reload.rs:207-242. This is the actual runtime safety net (not the symlink rollback). Preserve: never `exit()` before a successful execv.

4. **Reload-recovery intents for in-process headless/swarm agents.** reload.rs:246-367, reload_recovery.rs. Headless swarm agents run in-process (comm_session.rs:683), so they ride the daemon execv and are re-spawned from persisted intents. This is ORTHOGONAL to the binary-path machinery and must be preserved regardless.

5. **Payload resolution through wrappers** (`resolve_binary_payload`, paths.rs:93-143). If the nix launcher is a `makeWrapper`/phase-run wrapper (the `_ai` shape in tests), mtime/identity comparisons must unwrap to the real payload. Keep this even in the collapsed model.

## Safe-to-delete (once reloads point at one fixed nix path)

These are correctness-critical ONLY because multiple self-managed pointers exist; a single atomic nix path makes them dead weight:
- `shared-server` channel entirely: `update_shared_server_symlink`, `promote_version_to_shared_server`, `advance_shared_server_if_tracking_stable`, `shared_server_tracks_stable`, `repair_stale_shared_server_channel`, `SharedServerRepair`, `shared_server_channel_is_current_enough`, `shared_server_binary_is_strictly_older_than`, `read_shared_server_version`, `shared_server_version_file`. **Correctness-critical TODAY, safe once collapsed** — all they do is reconcile drift between independently-moving pointers, which cannot happen with one atomic path.
- `stable`/`current` channel symlinks + markers: `update_stable_symlink`, `update_current_symlink`, `stable_version_file`, `current_version_file`, `read_stable_version`, `read_current_version`, `version_matches_installed_channel`. **Cosmetic/A once collapsed.**
- `canary` channel: `update_canary_symlink`, `canary_binary_path` — only debug + selfdev-fallback callers. **Safe/cosmetic.**
- Version store: `install_binary_at_version`, `install_version`, `version_binary_path`, `~/.jcode/builds/versions/` — **safe ONLY if the fixed path is itself atomic (nix result). Not safe if pointing at target/selfdev directly (see Must-preserve #1).**
- The whole pending-activation/canary rollback (`PendingActivation`, `set_pending_activation`, `complete/rollback_pending_activation_for_session`, `reconcile_stale_pending_activation`, manifest canary fields): **safe to delete IF you accept "no automatic revert of a bad self-dev build; fix source + rebuild + `nix run`/reload".** This is a deliberate policy call, not a mechanical one. Today it only restores channel symlinks, which won't exist.
- Subsystem A entirely (update.rs GitHub/source install, `advance_shared_server_if_tracking_stable` callers, `repair_stale_shared_server_after_update_check`, `install_local_release`, `install_main_source_update_blocking`): **[A] dead for the hard fork** per the brief (no curl|sh users).
- `update_launcher_symlink*` — under nix the launcher is nix-owned; already a no-op when `JCODE_NIX_MANAGED` set (paths.rs:495).

## Open questions / ambiguity to flag

1. **Atomicity of the collapsed path is non-negotiable.** The brief suggests `target/selfdev/jcode` OR a nix `result`. These are NOT equivalent: `result` is atomic; `target/selfdev/jcode` is written in place by cargo and is NOT. If the maintainer keeps a daily *self-dev cargo hot-reload loop* (they do), pointing the DAEMON's reload at `target/selfdev/jcode` reintroduces the truncated-binary race the version store was built to prevent. Recommendation: either (a) keep a minimal atomic-publish step (stage+fsync+rename to ONE fixed path, no versions/ dir, no channels), or (b) have selfdev build via `nix build` and reload `result`. A bare `target/selfdev/jcode` exec target is the risky option.

2. **Self-dev branch bypasses `nix_managed_launcher_override`.** Every resolver returns the override only when `!is_selfdev_session` (paths.rs:555). The maintainer works almost exclusively in self-dev. So flipping `JCODE_NIX_MANAGED` alone does NOT collapse the self-dev path — that path still uses `current`/`canary`/repo builds. The collapse must ALSO change the selfdev branch, or the self-dev reload must be redefined to target the fixed path.

3. **Blast-radius isolation is real but arguably obsolete.** The `shared-server` slower channel exists so one dev's dirty build doesn't take out a daemon shared by all clients. For a single-user hard-fork maintainer, "all clients" is themselves, so the isolation may be worthless. Confirm there is no multi-user/shared-daemon scenario before deleting it.

4. **`JCODE_MIGRATE_BINARY` migration UX** (debug_cmds.rs:1083, hot_exec.rs:59) lets a session drop back to `stable` on demand. Deleting `stable_binary_path` removes this "escape hatch to last good build." Under nix the equivalent is `nix run <pinned-older-rev>`; confirm that is acceptable.

5. **Is `JCODE_NIX_MANAGED` actually the intended activation switch?** It is 100% dormant today (Q4). The cleanest collapse is: set `JCODE_NIX_MANAGED=1` in the nix wrapper AND redefine selfdev to also honor a fixed target. The scaffolding is already there; it was written in anticipation of exactly this move (paths.rs:528-562 comments). This strongly suggests the collapse is the maintainers' own intended direction.

## Verdict

**Safe to collapse, with one hard constraint.** Subsystem B's channel/version machinery (stable/current/shared-server symlinks, `~/.jcode/builds/versions/`, advance/repair/tracks-stable) exists almost entirely to reconcile drift between multiple independently-moving self-managed pointers and to isolate a shared daemon from dirty local builds. A single atomic nix `result` path eliminates that entire bug class, and the `nix_managed_launcher_override` scaffolding (dormant because nothing sets `JCODE_NIX_MANAGED`) already implements the single-fixed-path model for the non-selfdev case. **Biggest risk: atomicity.** The version store's real load-bearing job is atomic publish (stage+fsync+rename) so a concurrently-reloading daemon never execs a half-written binary; a nix `result` preserves this, but pointing reloads directly at a cargo-written `target/selfdev/jcode` does NOT and reintroduces the truncated-ELF race. **Second risk: the self-dev branch bypasses the nix override**, so the collapse must explicitly redefine the self-dev reload target, not just flip the env var. Deleting the canary/pending-activation rollback is a policy decision (you lose auto-revert of a bad build in favor of "rebuild / nix run older rev"), not a correctness blocker.
