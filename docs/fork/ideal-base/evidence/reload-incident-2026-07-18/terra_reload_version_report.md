# Reload-version incident report

**Scope:** read-only source and artifact investigation. No repository files were changed and no build/reload was run.

**Repository inspected:** `/Users/jrudnik/labs/jcode`, `HEAD a87c5f27128f9a1634d5a0613c05ddfc198a3f45`.

## Executive conclusion

The forced reload did not select a binary by comparing the request hash (`923c6353e`) to repository `HEAD` or to `target/selfdev/jcode`. It selected a reload *channel target* at daemon-side execution time. At **21:38:16.289**, that target was `~/.jcode/builds/shared-server/jcode`, whose symlink and marker still pointed to `923c6353e-dirty-5a0f07fa7495`. Therefore the daemon re-exec'd the old binary.

The `a87c5f271` version directory was created **later**, at **21:38:27**, by a failed local publish/smoke-test attempt. The staged executable was a **zero-byte hard link** to `target/selfdev/jcode`, then its smoke test failed with invalid/empty JSON. The publication path had not reached either `update_current_symlink` or `update_shared_server_symlink`, hence neither channel marker moved. The directory is evidence of a failed staging attempt, not a usable fresh selfdev artifact that the earlier reload should have selected.

The non-force command's “already running newest” claim is an mtime-only directional decision, not a Git/HEAD/hash/manifest comparison. It compares the running payload’s filesystem mtime to channel candidates, treating any uncertainty as “not newer.” A newer repository commit or a newly-created-but-invalid/zero-byte artifact is not sufficient.

---

## Incident timeline and corroborating evidence

| Time | Observation | Evidence |
|---|---|---|
| 21:38:02.274 | A non-forced reload request arrives. | `~/.jcode/logs/jcode-2026-07-18.log:207352` |
| 21:38:02.275 | Server skips it: `no strictly-newer binary`. | log `:207354` |
| 21:38:15.413 | The later forced reload request arrives. The 38-byte request is consistent with an explicit `force` field versus the earlier 39-byte non-force serialization, and the ensuing code path bypasses the skip gate. | log `:207673-207675`; source below |
| 21:38:15.420 | Server sends and receives the reload signal with `hash=923c6353e`, `prefer_selfdev_binary=false`. | log `:207675`, `:207677` |
| 21:38:16.289 | Daemon exec target is logged as `shared-server`: `~/.jcode/builds/shared-server/jcode`. | log `:207741` |
| 21:38:16.303 | The replacement process starts. | log immediately following `:207741` |
| 21:38:27 | `versions/a87c5f271/` and `target/selfdev/jcode` have mtime 21:38:27. The binary is zero bytes. | `stat` inspection; `jcode.source.json` records clean `a87c5f271`, fingerprint `43e6fa7dd0bf623f` |
| 21:38:27.340 and .349 | Two attempts report: `Binary smoke test for .../versions/a87c5f271/jcode returned invalid JSON: EOF...`. | log `:208479-208480` |

Current on-disk activation evidence is consistent with the above:

* `~/.jcode/builds/shared-server/jcode` -> `.../versions/923c6353e-dirty-5a0f07fa7495/jcode`
* `~/.jcode/builds/shared-server-version` = `923c6353e-dirty-5a0f07fa7495`
* `~/.jcode/builds/current/jcode` and `current-version` are also `923c6353e-dirty-5a0f07fa7495`.
* `~/.jcode/builds/versions/a87c5f271/jcode` is a zero-byte file, same inode as `target/selfdev/jcode`, confirming the installer used the hard-link branch.
* `manifest.json` retains canary `923c6353e-dirty-5a0f07fa7495` and its old pending activation. It contains no completed `a87` activation.

## 1. Forced `jcode server reload --force`: target decision and why old code won

### CLI-to-server path

1. `src/cli/dispatch.rs:137-140` dispatches `ServerCommand::Reload { force, json }` to `commands::run_server_reload_command(force, json)`.
2. `src/cli/commands.rs:2119-2125` documents the semantic distinction: non-force requires a newer candidate, force reloads unconditionally.
3. `src/cli/commands.rs:2177-2204` first calls `repair_stale_shared_server_channel()` on the *client* as best effort, then calls `client.reload_with_force(force)`.
4. The server protocol handler dispatches `Request::Reload { id, force }` to `handle_reload` (`crates/jcode-app-core/src/server/client_lifecycle.rs:1745-1750`).
5. `crates/jcode-app-core/src/server/client_session.rs:721-741` makes the sole force decision: only `!force && !server_has_newer_binary()` produces the skip. Force bypasses this condition.
6. `client_session.rs:746-765` obtains `(triggering_session, prefer_selfdev_binary)`. The normal result is `(session-id, agent_guard.is_canary())`; lock contention falls back to `false` at lines 751-761.
7. Crucially, `client_session.rs:787` sets the signal hash to **the running daemon build’s compile-time** `jcode_build_meta::GIT_HASH`, not repo HEAD and not the client executable hash. The incident log consequently reports `923c6353e`.
8. `client_session.rs:788-789` sends this metadata through `send_reload_signal`.

### Daemon-side binary selection

1. `crates/jcode-app-core/src/server/reload.rs:176-178` takes `signal.prefer_selfdev_binary` and calls `reload_exec_target(prefers_selfdev)`.
2. The signal hash is written to reload state and logs, but **is not passed to `reload_exec_target`**. The hash therefore cannot select `versions/<hash>/jcode`.
3. `crates/jcode-app-core/src/server/util.rs:85-148` resolves a target, applies the no-downgrade mtime guard, and returns it to the exec code.
4. Its helper, `util.rs:164-185`, calls `server_update_candidate` for both session flavors and chooses the candidate with the newest payload mtime. `server_update_candidate` is only `build::shared_server_update_candidate(...)` (`util.rs:53-54`).
5. `crates/jcode-build-support/src/paths.rs:602-624` defines that candidate: `shared-server` is the approved primary target, falling back to `stable`, then `current_exe`. For selfdev, a present shared-server candidate is accepted directly.
6. `reload.rs:178-209` logs and `exec`s that returned path.

**Applied result:** At the target-selection moment, the channel’s only selected candidate was the old `shared-server` link. The incident’s own log proves this exact result at `:207741`. The later `a87` directory did not yet exist, and in any event was not on `shared-server`.

## 2. Who created `versions/a87c5f271`, and why shared-server/version did not update

### Identified creation mechanism

The observed artifact exactly matches `publish_local_current_build_for_source`:

* `crates/jcode-build-support/src/lib.rs:766-807` finds `target/selfdev/jcode`, validates it, obtains `install_binary_at_version`, writes `.source.json`, smoke-tests the installed file, and **only then** updates `current` at lines 796-797.
* `lib.rs:275-299` creates `versions/<version>/jcode`, attempting `std::fs::hard_link(source, dest)` before copying. The matching inode of `target/selfdev/jcode` and `versions/a87c5f271/jcode` proves this branch ran.
* The `a87` `.source.json` identifies the creator’s intended source state: clean `a87c5f271`, source fingerprint `43e6fa7dd0bf623f`.
* The two smoke-test errors at log `:208479-208480` prove the attempt stopped on the zero-byte artifact. This is after the forced reload completed, not before it.

The log does not include a durable command name or caller identity for the two smoke tests, so it cannot distinguish conclusively between the debug `reload` command and the selfdev tool’s publishing step from this evidence alone. Both paths call `publish_local_current_build_for_source`:

* Debug command: `crates/jcode-app-core/src/server/debug_command_exec.rs:579-596`.
* Selfdev tool: `crates/jcode-app-core/src/tool/selfdev/reload.rs:275-301`.

The debug command’s immediately following calls to `update_shared_server_symlink` and `update_canary_symlink` are at `debug_command_exec.rs:596-597`; they are unreachable after the publish/smoke error. The selfdev tool similarly cannot reach its `update_shared_server_symlink` at `tool/selfdev/reload.rs:320-328` if publishing returns an error.

### Why links and markers stayed old

* `publish_local_current_build_for_source` updates `current` only after the post-install smoke/identity checks (`lib.rs:787-797`). The smoke test failure precluded that.
* `update_current_symlink` is the only current marker update; it atomically swaps the channel link and writes `current-version` (`lib.rs:752-755`). It was not reached.
* `update_shared_server_symlink` atomically swaps `shared-server` and writes `shared-server-version` (`lib.rs:760-765`). Neither candidate caller reached it after the error.
* The persistent `pending_activation` is old state from `2026-07-18T05:45:12Z`, not an activation record for a87. It neither publishes a build nor determines `server reload` target selection. It is set/cleared by `lib.rs:217-276` and read by selfdev lifecycle recovery.

## 3. Meaning and callers of `prefer_selfdev_binary`; server reload versus debug reload

`ReloadSignal` carries `prefer_selfdev_binary` (`crates/jcode-app-core/src/server/reload_state.rs:417-425`). It is a target-resolution *preference*, not a requested hash and not a directive to exec `target/selfdev/jcode`.

### Values by caller

| Caller | Value | Basis |
|---|---:|---|
| `jcode server reload [--force]`, through remote client session | `agent_guard.is_canary()` | `client_session.rs:746-750` |
| Same path when agent lock is busy | `false` | `client_session.rs:751-761` |
| Selfdev reload tool | `true` | `tool/selfdev/reload.rs:342-347` |
| Debug command `reload` | `false` | `debug_command_exec.rs:608` |

The daemon uses it in `reload.rs:176-178` as an argument to `reload_exec_target`. Current target code then considers both flavors (`util.rs:164-185`) so a strictly newer candidate from the other flavor can win. Thus it is a tie/order preference, not a guarantee that a selfdev binary will be selected.

### Does server reload skip publishing/symlinks?

**Yes.** `jcode server reload` only does the stale shared-server repair, sends a protocol reload request, waits for handoff, and reports (`src/cli/commands.rs:2177-2273`). It does not call `publish_local_current_build_for_source`, `update_current_symlink`, `update_canary_symlink`, or `update_shared_server_symlink` directly.

The debug `reload` path does all of these before sending the signal:

* obtains `target/selfdev/jcode` (`debug_command_exec.rs:579-590`)
* publishes it to `versions/<source-version>` (`:592-595`)
* smoke-tests it (`:595`)
* moves `shared-server` (`:596`) and canary (`:597`)
* writes manifest canary state (`:599-602`)
* finally sends the signal (`:608`)

This is exactly why an explicit debug/selfdev reload can activate a new selfdev artifact whereas `server reload` cannot. It also explains why a failed publish leaves an immutable partial directory but preserves old activation links.

### Why client-side repair did not change shared-server here

`src/cli/commands.rs:2177-2204` calls `repair_stale_shared_server_channel`. However `crates/jcode-build-support/src/lib.rs:890-938` deliberately refuses repair when the previous shared-server marker is not a release-channel marker (`:910-916`). The incident marker was `923c6353e-dirty-5a0f07fa7495`, so repair correctly treated it as a deliberate selfdev pin and returned `AlreadyCurrent`; it did not repoint to stable.

## 4. Why non-force said “already newest” despite newer repo HEAD

The non-force condition is entirely `!server_has_newer_binary()` (`client_session.rs:721`). `server_has_newer_binary` is `crates/jcode-app-core/src/server/util.rs:371-428`:

1. It resolves the actual running payload using `current_exe` and `build::resolve_binary_payload` (`:397-405`).
2. It requests candidates for **both** flavors (`:407-412`), which ultimately are shared-server/stable/current fallback channel paths.
3. It compares modified times via `newer_binary_available` (`:414-428`).
4. It intentionally does **not** compare Git hash, repo HEAD, `manifest.canary`, `pending_activation`, `current-version`, or `shared-server-version` identity. Its comments at `:372-396` state that mtime is the only directional ordering for dev builds and uncertainty means no update.

At 21:38:02.275, a non-force request logged precisely `no strictly-newer binary` (`:207354`). In the activation state then visible to this code, shared-server was the daemon’s own old selfdev version; repository `a87` was not a promoted candidate. So the output was accurate for the stated mtime/channel predicate but misleading to someone expecting “newest repository commit.”

## Minimal fix proposal, not implemented

1. **Make `server reload` intent explicit.** Either document/rename its behavior as “restart onto the already-approved `shared-server` target,” or add an opt-in `--publish-selfdev`/`--from <path>` mode that validates, stages, smoke-tests, atomically promotes, then signals. Do not silently make ordinary server reload read arbitrary `target/selfdev/jcode` because that breaks the existing shared-daemon safety boundary.

2. **Prevent partial immutable version artifacts.** Stage `install_binary_at_version` into a temporary version file or directory; run the smoke/identity checks there; rename atomically into `versions/<version>` only after success. On failure, remove the temporary staging artifact. This prevents a zero-byte `versions/a87.../jcode` from looking published.

3. **Add machine-readable target diagnostics.** Before exec, record/log the selected candidate, resolved payload, mtime, current executable mtime, each competing candidate, and rejection reason. The existing final `Server: exec'ing into shared-server` log is helpful but lacks why another candidate was absent/rejected.

4. **Improve non-force messaging.** Replace “newest binary” with “no newer approved reload candidate by mtime” and include channel/version path. This accurately reflects the predicate and avoids implying a repo-HEAD comparison.

5. **Surface stale pending activation separately.** On startup/doctor, flag an old `pending_activation` that does not correspond to current/canary/shared-server markers. Do not let it alter reload selection, but make it diagnosable and safely recoverable through the existing session-specific completion/rollback APIs.

These changes preserve the crucial safety invariant: a shared daemon must not automatically upgrade to any transient local selfdev binary merely because it exists.
