# Independent R01 review: runtime build identity and reload authority

Date: 2026-07-15
Worktree: `/Users/jrudnik/labs/jcode-seam-r01`
Mode: read-only repo review. I did not read `/tmp/jcode-r01-opus-review.md`; a final status check showed it exists, but it was not opened. One broad early grep surfaced snippets from checked-in responsibility review files, including Opus-named paths; I excluded those snippets from this review and based conclusions on source, `RESPONSIBILITIES.md`, `PRESCREEN.md`, git commands, and local tests only.

## Verdict

**Disposition: compose, with fork R01 identity/reload authority as the base and upstream reload/session recovery only as a dependency.**

The fork has the materially relevant R01 machinery: source fingerprints, binary metadata validation, current/stable/shared-server channel markers, pending activation, Nix/shared-server reload candidate selection, protocol/build handshake, and tests for the stale-daemon class. Upstream has overlapping reload/session files and must not be ignored, but the evidence I reproduced does not show upstream owns canonical executable/source/build identity or the stale-daemon fix.

**Important prerequisite before pilot:** define exactly what R03A/R04 carry when R01 says "one runtime identity." Today R01 has a richer identity (`version_label`, `source_fingerprint`, channel markers), while R03A subscribe/handshake carries only `protocol_version` and `build_hash` (`jcode_build_meta::GIT_HASH`). Dirty self-dev builds from the same commit can have different source fingerprints but advertise the same handshake build hash. That may be acceptable if `build_hash` is only a compatibility signal, but then it is not the canonical runtime identity. The pilot must either carry `version_label`/`source_fingerprint` through R03A/R04 observables or explicitly document that R03A carries a compatibility identity, not the full R01 identity.

Confidence: **medium-high** for disposition, **medium** for pilot readiness because I did not exercise a live user daemon and the dirty-build identity gap remains a semantic question.

## Checkpoint 1: fixed refs and scope

Reproduced refs and ancestry:

```text
HEAD 8848f2d54f67f9a5a1de76bace9666c78036e116
baseline 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4
upstream 802f6909825809e882d9c2d575b7e478dce57d3b
merge-base 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d
base-is-ancestor-of-fork
base-is-ancestor-of-upstream
```

`git status --short --branch` remained clean for tracked files: `## recovery/seam-r01-20260715`.

R01 scope in the approved map is canonical executable, source fingerprint, build hash, current/published/pending targets, reload-target selection, and meaning of identity carried elsewhere. It excludes wire encoding/compatibility verdicts, client handoff, and release publication except dependencies (`docs/fork/recovery/RESPONSIBILITIES.md:21-24`). The same document makes R01 the highest full-review seam because of the stale-daemon incident and four-way identity risk (`RESPONSIBILITIES.md:50-53`). Cross-seam invariant 1 says manifest/launcher state, daemon reload state, R03A subscribe fields, and R04 restart snapshots must describe the same build/channel identity, with R01 owning meaning (`RESPONSIBILITIES.md:61-68`).

## Checkpoint 2: stale-daemon incident summary and why R01 is real

`PRESCREEN.md` says R01 matched selfdev, reload, build identity/hash/registry, server runtime/socket/spawn paths (`PRESCREEN.md:47-53`). Divergence was substantial: R01 had 29 fork-changed files, 17 upstream-changed files, and all 17 upstream-matched paths overlapped; reload/runtime symbols diverged on both sides (`PRESCREEN.md:64-75`).

The stale-daemon incident summary is direct R01 evidence: on 2026-07-14, the Nix binary, selfdev current build, stable channel, and live daemon diverged; reload reported "already newest" while the daemon still mapped an old executable; forced reload changed the mapped build while preserving sessions (`PRESCREEN.md:110-119`). The pre-screen explicitly leaves open whether current code still reproduces the live daemon incident (`PRESCREEN.md:178-180`).

## Checkpoint 3: fork/upstream symbol-level authority

I reproduced a path-focused fork/upstream diff from merge base for build/reload/selfdev/protocol/server/TUI identity areas. Results:

```text
fork R01 candidate paths: 46
upstream R01 candidate paths: 21
overlap: 21
```

The overlap includes reload/session files such as `server/client_lifecycle.rs`, `server/reload_recovery.rs`, `server/reload_state.rs`, `tool/selfdev/mod.rs`, protocol wire files, TUI backend, and tests. But the fork adds or materially changes the core R01 authority files that upstream did not in this slice:

- `crates/jcode-build-support/src/paths.rs`: Nix-managed launcher override, shared-server candidate selection, wrapper payload resolution, preferred reload candidate behavior.
- `crates/jcode-build-support/src/source_state.rs`: source fingerprint/version label model.
- `crates/jcode-app-core/src/server/handshake.rs`: server-side build/protocol identity verdict.
- `crates/jcode-tui/src/tui/backend.rs`: TUI advertises protocol/build identity on subscribe.
- `crates/jcode-protocol/src/wire.rs`: `HandshakeCompatibility`, subscribe identity fields, and handshake verdict event.

Fork-authority challenge: the fork has the most R01-relevant code, but it must not self-certify by comments or tests alone. It still uses mtime as the directional reload order and `GIT_HASH` as protocol identity, while richer source fingerprints exist elsewhere. That is a semantic split, not merely an implementation detail.

Upstream-authority challenge: upstream touched overlapping reload/session files, so its recovery/handoff behavior may still be important for R04/R03A dependencies. However, my diff did not show upstream owning the R01 source/build/channel identity model or the stale shared-server/Nix repair path. Upstream alone is not sufficient authority for R01 disposition.

## Checkpoint 4: known runtime-identity sources

Known identity sources and protected observables:

1. **Compile-time binary identity:** `jcode_build_meta::GIT_HASH` and version JSON. `read_binary_version_report` runs `binary version --json` and parses `BinaryVersionReport` (`crates/jcode-build-support/src/lib.rs:257-278`). Publishing rejects a binary whose reported `git_hash` differs from the source short hash (`lib.rs:292-314`).
2. **Source state:** `current_source_state` hashes the full commit, porcelain status, binary diff, and untracked file fingerprints into `fingerprint`; dirty builds become `<short>-dirty-<fingerprint-prefix>` version labels (`crates/jcode-build-support/src/source_state.rs:91-137`). `BuildInfo` stores `source_fingerprint` and `version_label` (`source_state.rs:214-228`).
3. **Dev binary metadata sidecar:** `write_dev_binary_source_metadata` writes `DevBinarySourceMetadata`; `validate_dev_binary_source_metadata` and `dev_binary_matches_source` require fingerprint/version/hash/dirty equality (`crates/jcode-build-support/src/lib.rs:242-255`, `408-448`).
4. **Build manifest:** `BuildManifest` stores stable, canary, canary session/status, history, last crash, and pending activation (`crates/jcode-build-support/src/lib.rs:47-67`). Pending activation can be completed or rolled back per session (`lib.rs:167-205`).
5. **Published/current/shared-server channel markers:** storage helpers define immutable version binaries and `stable`, `current`, `shared-server`, and `canary` paths plus marker files (`crates/jcode-build-support/src/storage_helpers.rs:6-67`, `94-140`). Channel symlink updates atomically swap the binary and write marker files (`crates/jcode-build-support/src/lib.rs:654-695`).
6. **Reload context:** `ReloadContext` records `version_before`, `version_after`, `session_id`, timestamp, and task context (`crates/jcode-app-core/src/tool/selfdev/mod.rs:70-83`); reload saves it after publishing/pending activation (`crates/jcode-app-core/src/tool/selfdev/reload.rs:328-345`).
7. **Reload marker:** `ReloadState` records request id, hash, phase, pid, timestamp, and detail in `jcode.reload` (`crates/jcode-app-core/src/server/reload_state.rs:424-469`).
8. **R03A subscribe identity:** `Request::Subscribe` carries `protocol_version` and `build_hash` (`crates/jcode-protocol/src/wire.rs:221-232`); TUI sends `PROTOCOL_VERSION` and `jcode_build_meta::GIT_HASH` (`crates/jcode-tui/src/tui/backend.rs:335-340`).
9. **Server handshake verdict:** server compares client protocol/hash against its own compiled identity and emits `HandshakeVerdict` only to clients that advertised protocol version (`crates/jcode-app-core/src/server/handshake.rs:23-69`).
10. **Reload exec observable:** server records `exec_start` with binary label, binary path, socket, and elapsed time before `replace_process` (`crates/jcode-app-core/src/server/reload.rs:155-180`).

## Checkpoint 5: current/published/pending targets

The fork establishes three separate target concepts:

- `current`: active local build marker and symlink (`storage_helpers.rs:32-35`, `59-62`; update in `lib.rs:682-687`).
- `stable`: installed/published stable release marker and symlink (`storage_helpers.rs:27-30`, `54-57`; update in `lib.rs:675-680`).
- `shared-server`: approved daemon channel marker and symlink (`storage_helpers.rs:37-40`, `64-67`; update in `lib.rs:689-695`).

Selfdev reload publishes the current source to an immutable version, smoke-tests the server binary, records the previous shared-server version, writes pending activation, then updates shared-server to the new hash (`crates/jcode-app-core/src/tool/selfdev/reload.rs:270-326`). Pending activation includes the source fingerprint and previous current/shared-server versions (`reload.rs:305-318`), and rollback restores both current and shared-server (`crates/jcode-build-support/src/lib.rs:185-205`). This is strong fork evidence for protected pending-target semantics.

## Checkpoint 6: reload-target selection

The current code separates client update candidate, shared-server candidate, and preferred reload candidate:

- Non-selfdev Nix-managed mode bypasses self-managed `builds/` shadow and resolves to the launcher/current executable (`crates/jcode-build-support/src/paths.rs:528-562`).
- `shared_server_update_candidate` intentionally avoids fast-moving `current`; it uses `shared-server` if current enough, otherwise stable/current exe fallback (`paths.rs:596-623`).
- `preferred_reload_candidate` uses the Nix-managed override, then considers channel candidate and newer repo build by payload mtime (`paths.rs:690-735`).
- `reload_exec_target` chooses the newest reload candidate across selfdev and non-selfdev flavors, resolves wrappers to payloads, strips Linux ` (deleted)`, and blocks strict downgrades to older candidates when current exe is available (`crates/jcode-app-core/src/server/util.rs:51-143`, `149-249`).

This directly addresses the PRESCREEN stale-daemon class more than upstream evidence does. The no-downgrade and cross-flavor candidate selection are especially relevant to "reload reported newest but daemon remained old." Tests assert that older candidates are blocked, selfdev can follow a fresh release, and normal user daemon detects and targets update after update (`crates/jcode-app-core/src/server/util.rs:621-674`, `696-760`, `808-918`).

Risk: the decisive order is filesystem mtime, not source fingerprint or semantic version. The code deliberately uses mtime because dev builds share base semver and hashes cannot order builds (`crates/jcode-app-core/src/server/util.rs:365-388`). That is a pragmatic reload authority, but it should be named as such. It is not a cryptographic build identity.

## Checkpoint 7: R03A/R04 identity carriage

R03A currently carries a compatibility identity, not the full R01 identity:

- `HandshakeCompatibility::evaluate` treats no advertised protocol as legacy compatible, protocol mismatch as reconnect, same protocol/different known hashes as reconnect, and unknown/uncomparable hashes as compatible (`crates/jcode-protocol/src/wire.rs:49-124`).
- TUI subscribes with `protocol_version` and `build_hash` only (`crates/jcode-tui/src/tui/backend.rs:335-340`).
- Server lifecycle evaluates and notifies on subscribe, but continues into session handling after emitting the verdict (`crates/jcode-app-core/src/server/client_lifecycle.rs:1338-1373`).

R04/reload state carries `hash` from the reload signal and marker, which for selfdev reload is `source.version_label` (`crates/jcode-app-core/src/tool/selfdev/reload.rs:284-349`; `crates/jcode-app-core/src/server/reload_state.rs:402-432`). That is closer to full R01 identity than the R03A `build_hash` field.

**Adversarial call:** invariant 1 in `RESPONSIBILITIES.md` says R03A subscribe fields and R04 restart snapshots must describe the same build/channel identity. On current source, R04 can carry `8848f2d-dirty-...` while R03A carries `8848f2d` only. If this is intentional, rename/define the fields to avoid false equivalence. If not intentional, add `version_label` and optionally `source_fingerprint` to the R03A handshake and verdict.

## Checkpoint 8: validation, decisive tests, and pilot prerequisites

Tests run successfully via `scripts/dev_cargo.sh` after it re-entered the Nix dev shell because plain `cargo` was not on PATH.

Passed validation:

```text
scripts/dev_cargo.sh test -p jcode-protocol --lib handshake_verdict -- --nocapture
7 passed

scripts/dev_cargo.sh test -p jcode-app-core --lib handshake -- --nocapture
6 passed

scripts/dev_cargo.sh test -p jcode-app-core --lib reload_target_tests -- --nocapture
6 passed

scripts/dev_cargo.sh test -p jcode-app-core --lib newest_reload_candidate_integration_tests -- --nocapture
4 passed

scripts/dev_cargo.sh test -p jcode-build-support --lib dev_binary_matches_source -- --nocapture
1 passed

scripts/dev_cargo.sh test -p jcode-build-support --lib pending_activation_can_complete_and_roll_back -- --nocapture
1 passed

scripts/dev_cargo.sh test -p jcode-build-support --lib shared_server_candidate -- --nocapture
5 passed

scripts/dev_cargo.sh test -p jcode-build-support --lib dirty_source_state_uses_fingerprint_in_version_label -- --nocapture
1 passed
```

These tests cover the local mechanics but are not sufficient as a pilot gate because they do not attach a real client to a restarted daemon and do not prove the R03A/R04 identity fields are semantically the same.

Cheapest decisive tests before pilot:

1. **Dirty-build identity round trip:** create two dirty builds from the same commit with different source fingerprints in temp `JCODE_HOME`; assert R01 canonical identity differs and R03A/R04 observables either carry that difference or explicitly classify it as non-compatibility identity. This is the cheapest test for the current semantic gap.
2. **Temp-daemon stale shared-server fixture:** no live user daemon. Spawn a throwaway daemon with temp socket/JCODE_HOME, install old shared-server and newer stable/current fixture binaries, connect a client, request reload, and assert the exec target label/path/version is the newer expected target.
3. **Nix-wrapper payload fixture at daemon boundary:** current tests cover payload resolution in build-support; add a server-level test that the same payload resolution is used by both `server_has_newer_binary` and `reload_exec_target` for Nix wrappers.
4. **Pending activation failure rollback:** extend existing pending activation test to cover `do_reload` failure after shared-server update but before readiness, verifying both current and shared-server markers roll back and the manifest clears pending activation.
5. **Handshake action test:** server emits incompatible verdict today, but client action is outside R01 scope. As a dependency, require one R03A/client test showing incompatible verdict causes reconnect/re-exec, not silent attach.

Pilot prerequisites:

- R01 ledger must define canonical identity fields: at minimum `version_label`, `source_fingerprint`, compiled `git_hash`, channel marker, executable path/payload, and whether identity is release, stable, current, shared-server, or selfdev.
- R03A must either carry those fields or document that `build_hash` is only a compatibility token. The pilot should not claim one runtime identity if R03A and R04 carry different granularities.
- R04 reload/restart snapshot must record the same canonical version label and source fingerprint used by R01 pending activation, plus exec target label/path.
- The pilot should run only under temp `JCODE_HOME` and temp socket, not the live user daemon.
- Existing trusted tests above should remain green; add the dirty-build round trip first because it is the most likely to expose a false identity equivalence.

## Gaps

- I did not read the forbidden `/tmp/jcode-r01-opus-review.md` and did not use any Opus conclusions.
- I did not run a live daemon or networked/user-secret flow, by instruction.
- I did not inspect every upstream commit in the R01 overlap. The path/symbol evidence is enough for disposition, but a later composition phase should still review upstream reload recovery hunks for R04 dependencies.
- Local tests built into `target/` and the wrapper installed git hooks in `.git/hooks`; tracked source remained clean.
- The biggest unresolved semantic gap is whether R01 canonical identity is source fingerprint/version label or compiled git hash/protocol compatibility. The current code uses both, but not as the same observable.
