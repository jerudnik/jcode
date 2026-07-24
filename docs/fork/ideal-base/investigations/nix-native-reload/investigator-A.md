# SUMMARY VERDICT

The collapse is LARGELY SAFE, and in nix-managed mode (JCODE_NIX_MANAGED set,
non-selfdev) the version store + channels are ALREADY bypassed today by
`nix_managed_launcher_override` (paths.rs:545-562) - so for that mode they are
already dead weight. The version store/channels are load-bearing ONLY for the
self-managed selfdev/update model. The SINGLE BIGGEST RISK is losing two real
safety properties that ride on the store: (1) smoke-test-before-activate (a
broken freshly-built binary never becomes the thing every process execs), and
(2) rollback-to-previous-known-good (the store retains a prior version + the
channel pointer can be repointed back). A naive single fixed path has neither:
if the just-built binary is broken, the running process survives only because
execv fails, and the NEXT spawn re-runs the same broken path. If the nix-native
target is JCODE_NIX_MANAGED (nix owns the launcher and nix generations provide
rollback + the build is CI/smoke-gated), both properties are covered by nix and
the collapse is safe. If the target is a bare `target/selfdev/jcode` or a
`result` symlink with no smoke gate and no generation rollback, you must
preserve a smoke-gate and a 2-slot (previous/current) fallback, or accept that a
bad build wedges every subsequent spawn.

Everything else (channel selection, cross-flavor newest-candidate scan,
downgrade guard, forced-stale refusal, dev_binary_matches_source staleness
check, canary/pending-activation manifest bookkeeping) is optimization/policy
that only exists because MULTIPLE candidate builds compete; a single fixed path
makes them moot. The daemon reload EXEC+reconnect machinery, the wrapper->payload
resolver, and the protocol/version handshake (the actual compatibility gate) are
orthogonal to the store and must stay.

---

# Nix-native self-update collapse: safety investigation (Investigator A)

Status: IN PROGRESS. Findings appended incrementally.

## Files read so far
- `src/cli/hot_exec.rs` (full)

## Q1 progress: reload target resolution (from hot_exec.rs)
- `hot_reload(session_id)` (`src/cli/hot_exec.rs:54`):
  - First checks env `JCODE_MIGRATE_BINARY` (`:59`); if set and exists, execs into that with `--no-update` (a migrate-to-stable path). Load-bearing? It's an escape hatch used by migration.
  - Otherwise resolves `(exe, _label) = build::preferred_reload_candidate(is_selfdev)` (`:80`). This is THE reload target for selfdev hot-reload.
  - Retries exec up to 3 times handling ENOENT (`:102-125`). NOTE: there is NO rollback to a previous version here; failure just returns Err and the caller presumably keeps running or exits.
- `hot_restart(session_id)` (`:35`): execs `std::env::current_exe()` (the SAME running binary), not the version store. So restart != reload.
- `hot_update(session_id)` (`:132`): download/install release path; uses `build::client_update_candidate(is_selfdev)` (`:158`) to pick exec target, falling back to downloaded `path`.
- `run_auto_update` (`:275`) and `run_update` (`:333`): git pull + cargo build --release + `build::install_local_release`, then exec `build::client_update_candidate(false)`.
- `repair_stale_shared_server_after_update_check` (`:404`) calls `build::repair_stale_shared_server_channel()`.
- `reload_server_after_update` (`:431`) execs `client_update_candidate` (or current_exe) with `server reload --force` to reload the daemon.

Observed: NO rollback-from-version-store logic in hot_exec.rs. "Recovery" from a bad exec is only the retry loop for ENOENT.

## Q1 answer (reload target resolution) - CONFIRMED from paths.rs

`preferred_reload_candidate(is_selfdev)` (`paths.rs:696-736`):
1. If `nix_managed_launcher_override` returns Some (JCODE_NIX_MANAGED set AND not selfdev), returns launcher / current_exe. (`:697-700`)
2. Else `candidate = client_update_candidate(is_selfdev)` (`:702`)
3. Compares repo build (target/selfdev or target/release) mtime vs candidate payload mtime, prefers repo build if strictly newer (`:704-735`). Uses `resolve_binary_payload` to compare payloads not wrapper scripts.

`client_update_candidate(is_selfdev)` (`paths.rs:564-594`) resolution order:
1. nix_managed_launcher_override (`:565`)
2. `current` channel symlink `~/.jcode/builds/current/jcode` (`:569`)
3. if selfdev: repo dev binary (target/selfdev or target/release) (`:573-579`), then `canary` channel (`:580`)
4. `launcher` (`~/.local/bin/jcode`) (`:585`)
5. `stable` channel (`:589`)
6. `current_exe()` fallback (`:593`)

`nix_managed_launcher_override` (`paths.rs:545-562`): gate = `is_externally_managed()` (JCODE_NIX_MANAGED env set, `:486-488`) AND NOT selfdev. Returns launcher_binary_path or current_exe, label "nix-managed". Explicitly BYPASSES the builds/ shadow. **This is already the nix-native path** and is documented as the fix for the "self-certifying-channel version-drift incident."

`shared_server_update_candidate(is_selfdev)` (`paths.rs:602-623`):
1. nix_managed_launcher_override (`:603`)
2. `shared-server` channel: if selfdev, always use it if it exists; else only if `shared_server_channel_is_current_enough()` (`:607-616`)
3. `stable` channel (`:618`)
4. current_exe fallback (`:622`)
- Note: deliberately does NOT follow `current`, so "local dirty self-dev builds stop taking out every client by accident" (`:596-601`).

KEY OBSERVATION: In nix-managed mode, ALL of these functions short-circuit to the launcher/current_exe and NEVER touch the version store or channels (for non-selfdev). So the version store + channels are ALREADY bypassed in the nix path. The channels/version-store are the NON-nix (self-managed) resolution machinery.

## Q4 partial (source-fingerprint matching) - from lib.rs + paths.rs
- `dev_binary_matches_source(binary, source)` (`lib.rs:897-908`): returns true only if the binary's `.source.json` sidecar exactly matches the SourceState (fingerprint, version_label, short/full hash, dirty). Doc (`:893-896`): "a stale-check must never fail a launch on its own; worst case it triggers a rebuild. Used by the self-dev launcher to auto-rebuild a stale binary." => OPTIMIZATION (staleness detection), not correctness gate.
- `current_source_state` (source_state, re-exported) computes source fingerprint from git state.
- `ensure_source_state_matches` used in run_selfdev_build (`paths.rs:402`) after build to detect source changed during build.
- `validate_dev_binary_matches_source` (`lib.rs:910-927`) is used in publish path (`publish_local_current_build_for_source` `:1167`) - it refuses to PUBLISH a binary whose git_hash disagrees with source. This is a publish-time correctness guard, tied to the version store publish flow. If version store deleted, this guard is deleted with it.

## Rollback machinery (Q2) - from lib.rs
- `rollback_pending_activation_for_session` (`lib.rs:254-274`): restores `current` and `shared-server` symlinks to `previous_*_version` from a PendingActivation record. THIS IS REAL ROLLBACK using the version store + channels.
- `reconcile_stale_pending_activation` (`lib.rs:327-374`): if a pending activation's initiating session died, verifies candidate (via `.source.json` sidecar) else rolls back symlinks. Uses `version_binary_path`, `read_current_version`, `read_shared_server_version`.
- `complete_pending_activation_for_session` (`lib.rs:236-252`).
- `PendingActivation` has `previous_current_version` and `previous_shared_server_version` fields => the version store retains PREVIOUS versions for rollback. Need to check who SETS pending_activation and whether hot_reload uses it. (hot_exec.rs did NOT reference it.)

NOTE: This rollback is NOT wired into hot_reload's exec path (hot_exec.rs has no rollback). Need to find where set_pending_activation is called and what consumes reconcile.

## Q3 answer (multi-process / shared-server coordination) - from reload.rs + util.rs + tool/selfdev/reload.rs

### The daemon reload exec target
- `reload::await_reload_signal` (`server/reload.rs:57-244`) is the daemon's reload loop. On a signal it: begins shutdown drain, persists recovery intents, graceful shutdown, then calls `super::reload_exec_target(prefers_selfdev, signal.force())` (`:183`) and `replace_process` (execv) into `<binary> serve --socket <socket>` (`:204-207`).
- `reload_exec_target` (`server/util.rs:88-101`) -> `resolve_reload_target` (`:110-128`) -> collects candidates via `collect_reload_target_candidates`, picks NEWEST exec candidate across BOTH selfdev flavors, applies a NO-DOWNGRADE guard vs current_exe mtime.
- `server_update_candidate` (`util.rs:56-58`) = `build::shared_server_update_candidate` -> the `shared-server` channel (or stable). So the daemon reload target IS the shared-server channel by design.

### What coordinates "all live processes reload onto the SAME new binary"?
- The daemon is a SINGLE long-lived process. Swarm agents are separate processes but they DO NOT each exec a binary on reload. The daemon reload = the daemon execs itself into the new binary; agents are gracefully checkpointed/shut down (`graceful_shutdown_sessions` `reload.rs:369`) and later reconnect/relaunch. So there is not an N-process simultaneous exec that needs a shared pointer at one instant.
- The `shared-server` channel exists so the daemon reloads a DELIBERATELY-PROMOTED binary, NOT the fast-moving `current` (which tracks every local dirty selfdev build). Doc `paths.rs:596-601`: "shared server should only run binaries that were explicitly promoted onto the shared-server channel (or stable), so local dirty self-dev builds stop taking out every client by accident." This is a POLICY/ISOLATION mechanism (which build the daemon runs), not a low-level "same binary at same instant" coordination.
- Client-side reload (`hot_exec.rs::hot_reload`) uses `preferred_reload_candidate` (client channels), a DIFFERENT resolution than the daemon's shared-server channel. So client and daemon can be on different binaries intentionally.

### advance_shared_server_if_tracking_stable / repair_stale_shared_server_channel
- `advance_shared_server_if_tracking_stable(version)` (`lib.rs:1241-1248`): during an UPDATE, moves shared-server forward to the new release ONLY IF it was tracking stable (not pinned to a self-dev build). Called in update.rs:333, 1096 BEFORE stable moves.
- `repair_stale_shared_server_channel` (`lib.rs:1281-1328`): a NEWER client that finds an OLDER daemon repoints shared-server -> stable so the forced reload has a strictly-newer target. Called from hot_exec.rs update path.
- Both are about keeping the daemon's promoted-binary pointer from drifting behind releases. They are the "which build should the long-lived daemon run" policy, tied entirely to the channel/version-store model.

### Verdict on Q3
The multi-process coordination does NOT require version labels to atomically move N processes. It requires a STABLE POINTER (shared-server channel) that:
(a) isolates the daemon from fast-moving local dirty builds, and
(b) can be advanced/repaired by updates and by newer clients.
In a nix-native single-path world where the daemon always execs the launcher (nix profile binary), `nix_managed_launcher_override` ALREADY bypasses shared-server entirely (`paths.rs:603`). So for nix-managed installs the shared-server channel is already dead code. The channel is load-bearing ONLY for the self-managed (non-nix) selfdev/update model.

## Q2 answer (rollback) - from tool/selfdev/reload.rs + lib.rs
- REAL rollback exists, but ONLY in the selfdev `do_reload` tool path (`tool/selfdev/reload.rs:240-434`), NOT in hot_exec.rs.
- Flow: publish build -> `set_pending_activation{previous_current_version, previous_shared_server_version, ...}` (`:309-318`) -> `update_shared_server_symlink(hash)` (`:322`) -> send reload signal -> wait ack -> `await_reload_handoff`.
  - On Ready: `complete_pending_activation_for_session` (`:394`).
  - On Failed/Idle/Waiting: `rollback_pending_activation_for_session` (`:401,411`) which RESTORES current + shared-server symlinks to previous versions (`lib.rs:254-274`).
- So rollback = "repoint the channel symlinks back to the previous KNOWN-GOOD version in the version store, so the next reload/spawn does not use the broken build." It depends on the version store retaining the PREVIOUS version binary.
- `reconcile_stale_pending_activation` (`lib.rs:327-374`, called from server.rs:1186) handles the case where the initiating session DIED mid-reload: verifies candidate via `.source.json`, else rolls back symlinks.
- IMPORTANT NUANCE: This rollback does NOT restore the RUNNING process. The daemon has already exec'd (or failed to exec). "Rollback" is forward-looking: it fixes the POINTER so subsequent processes use the good binary. If exec of a bad binary fails, execv returns and the OLD process image is still running (that is the immediate safety); the symlink rollback prevents the NEXT attempt from re-selecting the bad build.

CROSS-CHECK NEEDED: In nix-native single-path, there is no "previous version" retained. If a freshly built single-path binary is broken, exec fails -> old process keeps running (still safe at that instant), BUT there is no known-good pointer to fall back to for the next spawn; the next spawn would re-run the same broken single path. So rollback-to-previous-version is a REAL capability that a naive single-path collapse loses. Whether it MATTERS depends on the smoke-test gating (install_binary_at_version smoke-tests before publish) which also goes away.

## Q4 answer (source-fingerprint matching) - FULL
Three related mechanisms; different criticality:

1. `dev_binary_matches_source(binary, source)` (`lib.rs:897-908`): pure staleness check. Used at selfdev launch (`src/cli/selfdev.rs:136-140`) to decide whether to AUTO-REBUILD before launching. Doc `lib.rs:893-896`: "a stale-check must never fail a launch on its own; worst case it triggers a rebuild." => OPTIMIZATION. If a single fixed path is always freshly built by the reload build step, this is redundant. NOT correctness-critical.

2. `ensure_source_state_matches(repo_dir, expected)` (`source_state.rs:140-150`): after a build, bails if the source fingerprint DRIFTED during the build (you edited files mid-build). Used in `run_selfdev_build` (`paths.rs:402`), selfdev launch (`selfdev.rs:153`), build_queue (`build_queue.rs:339,357`). => CORRECTNESS-ADJACENT but tied to the publish/version-label flow: it guarantees the version label you attach matches what you built. In a single-path world you would still want "did the tree change during build" but you would NOT need it to guard a version-store publish. Reduced importance, not gone.

3. `validate_dev_binary_matches_source` (`lib.rs:910-927`) + `validate_binary_version_matches_source_report` (`:752-775`): PUBLISH-TIME guard. Refuses to install a binary into `versions/<label>` whose embedded git_hash disagrees with the source, or whose `.source.json` disagrees. This is what stops "publishing a binary that does not match its label." It is correctness-critical FOR THE VERSION STORE MODEL (mislabeled versions would poison reload/rollback selection). If the version store is deleted, this guard is deleted with it, and it is only meaningful because labels are used to select binaries.

Conclusion for Q4: The fingerprint machinery is correctness-critical ONLY inside the label-based version-store selection model (prevents selecting/publishing a mislabeled binary). Collapse to a single fixed path that is "whatever was just built" makes labels irrelevant, so most of this becomes moot. The one genuinely useful residual is `ensure_source_state_matches` (detect tree-changed-during-build), which is cheap to keep independent of the store.

## Q5: What breaks + classification

Reload target sources by mode:
- NIX-MANAGED (JCODE_NIX_MANAGED set), non-selfdev: `nix_managed_launcher_override` short-circuits ALL of client_update_candidate / shared_server_update_candidate / preferred_reload_candidate to the launcher (nix profile). Version store + channels ALREADY UNUSED. (`paths.rs:545-562,565,603,698`)
- SELF-MANAGED (no nix) or selfdev sessions: version store + channels ARE the resolution mechanism.

(a) Delete `~/.jcode/builds/versions/` + `install_binary_at_version`:
- `install_local_release` (`lib.rs:1373`), `publish_local_current_build_for_source` (`lib.rs:1157`), `update.rs:328,1092`, `install_version` (`lib.rs:1391`) all install into versions/. They feed the channel symlinks. CORRECTNESS-CRITICAL for the self-managed model (channels are symlinks INTO versions/; without versions/, channels dangle). If also removing channels + going single-path, these become dead. Safe to delete AS A SET with (b)+(c), not piecemeal.
- Smoke-test-before-publish (`install_binary_at_version_in_builds_dir` -> `smoke_test_staged_binary_for_install` `lib.rs:407,528-535`): this is a REAL safety gate (rejects a binary that can't even run `version --json`, and for server smoke `smoke_test_server_binary`). Collapsing to a single path removes the gate: a broken freshly-built binary would be exec'd directly. CORRECTNESS-RELEVANT (see Q2/rollback).

(b) Delete stable/current/shared-server channel symlinks + update/repair funcs:
- `update_stable_symlink`/`update_current_symlink`/`update_shared_server_symlink` (`lib.rs:1136-1155`), `advance_shared_server_if_tracking_stable` (`:1241`), `repair_stale_shared_server_channel` (`:1281`), `shared_server_tracks_stable`, `promote_version_to_shared_server`.
- Consumers: `hot_exec.rs` (repair on update), `update.rs`, `tool/selfdev/reload.rs` (publish+promote+rollback), `dispatch.rs:1196` (spawn server via shared_server_update_candidate), `server/util.rs` (reload_exec_target via shared_server_update_candidate), `debug_command_exec.rs`.
- The `shared-server` channel's PURPOSE (isolate daemon from fast-moving `current`/dirty selfdev; `paths.rs:596-601`) is a POLICY that only matters because `current` moves on every dirty build. If the single reload path is a deliberately-built artifact (target/selfdev or nix result), that isolation policy is moot. => cosmetic/optimization IN A SINGLE-PATH WORLD, but load-bearing in the current multi-channel world.
- `forced_stale_shared_server_refusal` (`util.rs:494`) + no-downgrade guard: these exist to stop a forced reload from DOWNGRADING onto a stale channel. In single-path world "downgrade" is impossible (only one path), so these are moot. cosmetic-in-single-path.

(c) Make reload target a single fixed path:
- Would replace `preferred_reload_candidate` (`paths.rs:696`), `client_update_candidate` (`:564`), `shared_server_update_candidate` (`:602`), `reload_exec_target`/`resolve_reload_target`/`collect_reload_target_candidates` (`util.rs:88,110,343`) with a constant. 
- BREAKS: rollback-on-failed-reload (Q2) because there is no "previous known-good version" pointer to restore. CORRECTNESS-CRITICAL if you rely on rollback. BUT note: rollback today only repoints symlinks; the running process safety on a bad exec is just "execv failed, old image still running." The single-path loss is: after a bad build, the NEXT spawn re-runs the same bad path. Mitigation: keep smoke-test-before-swap OR keep a 2-slot (previous/current) pointer. This is the SINGLE BIGGEST RISK.

## Load-bearing (must preserve or replace)
1. Smoke-test-before-activate (`smoke_test_binary`/`smoke_test_server_binary`, `lib.rs:741,1023`) - gates a broken binary from becoming the thing every process execs. In single-path collapse this protection vanishes unless preserved.
2. Rollback-to-known-good (`rollback_pending_activation_for_session` + `previous_*_version`, `lib.rs:254-274`; `reconcile_stale_pending_activation` `:327`). A naive single fixed path has no previous-good fallback; the next spawn re-runs the bad binary. Needs a replacement (2-slot pointer, or nix generation rollback).
3. `nix_managed_launcher_override` (`paths.rs:545`) - the ALREADY-nix-native path. This IS the target shape; keep it.
4. `resolve_binary_payload` wrapper->payload resolution (`paths.rs:93`) - needed as long as the launcher can be a nix/makeWrapper wrapper. Keep regardless.
5. The daemon reload EXEC + reconnect mechanism itself (reload.rs, reload_state.rs, recovery intents) - orthogonal to where the binary comes from. Keep.
6. `ensure_source_state_matches` (tree-changed-during-build guard) - cheap, keep independent of store.

## Safe-to-delete (pure optimization/legacy in a single-path world)
1. `stable`/`current`/`shared-server`/`canary` channel symlinks and their update/repair/advance functions (`lib.rs:1114-1400`) - ONCE the single path replaces channel resolution. These are the "which of several builds" policy that a single path eliminates.
2. `client_update_candidate`/`shared_server_update_candidate`/`preferred_reload_candidate` multi-source resolution (`paths.rs:564,602,696`) - collapse to constant/launcher.
3. `reload_exec_target`'s cross-flavor newest-candidate scan + downgrade guard + forced_stale refusal (`util.rs:88-520`) - only meaningful with multiple candidates.
4. `dev_binary_matches_source` staleness check (`lib.rs:897`) - if the build step always produces the single path fresh.
5. `BuildManifest` canary/stable/pending-activation bookkeeping (`lib.rs:117-374`) - if rollback is replaced by nix generations / smoke-gate.
6. `version_matches_installed_channel`, `shared_server_channel_is_current_enough`, `is_release_channel_marker` (`paths.rs:625,664; lib.rs:1330`) - channel-marker helpers.

## Open questions
- Does anything OUTSIDE selfdev rely on the daemon running a DIFFERENT binary than a fresh client (i.e. is the shared-server-vs-current split ever intentionally used in production/non-selfdev)? In nix-managed mode it is already bypassed, so likely no. But confirm no non-nix production user depends on `stable` lagging `current`.
- Is the maintainer's nix-native target ALWAYS JCODE_NIX_MANAGED (launcher owns binary), or a `result` symlink that still needs rollback? If the latter, rollback replacement (nix generation / 2-slot) is required, not optional.
- The smoke test also runs a full `serve` protocol handshake (`smoke_test_server_binary`, unix only). Losing it means a binary that boots but can't bind the socket would be exec'd by the daemon and could wedge. Confirm whether nix build/CI already gates this.
- `hot_restart` and `hot_update` exec `current_exe`/downloaded path directly and never touched the version store; these are already single-path-ish. Confirm they are unaffected (they appear to be).
