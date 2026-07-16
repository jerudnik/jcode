# R01 full-seam review (Opus) — Runtime build identity and reload authority

- Worktree: `/Users/jrudnik/labs/jcode-seam-r01` at `8848f2d54f67f9a5a1de76bace9666c78036e116`, read-only. No repo/ref/branch/worktree/stash modifications.
- Behavioral baseline: fork `7ff4fc6be`; upstream `802f690982`; merge base `631935dd1`. Verified: `merge-base(fork,upstream)==631935dd1`, base is ancestor of `8848f2d5`, fork-baseline is ancestor of `8848f2d5`.
- Scope: RESPONSIBILITIES.md R01 only — canonical executable/source/build identity, current/published/pending targets, reload-target selection, and the meaning of identity carried through R03A/R04. Wire compatibility verdicts, client handoff, and release publication treated only as dependencies.
- Did **not** read or search for any Grok R01 artifact.
- Budget: 8 decisive checkpoints used. One planned test (server reload unit test) was blocked by a concurrent build lock; narrowed rather than expanded.
- Confidence: **high** on divergence localization and on the incident being encoded as passing invariants; **medium-high** on disposition; **medium** on the four-way identity consistency being fully closed (not exhaustively proven).

## What R01 owns (observables), grounded in code

R01's authority is "which build/source a long-lived process is, and which build a reload may switch it to." The concrete protected observables:

1. **Canonical build/source identity** — compile-time, single-source.
   `crates/jcode-build-meta/src/lib.rs:9-33`: `VERSION`, `GIT_HASH`, `GIT_DATE`, `SEMVER`, `UPDATE_SEMVER`, `BUILD_SOURCE_DIR` (answers "which checkout produced the running binary", G4 in `SELFDEV_NIX_DAEMON_DIVERGENCE.md`), `is_release_build()`. These are `pub const` re-exports of `cargo:rustc-env` values, so every crate reads identical identity. This is the intended single source of truth.
2. **Identity value types** — `crates/jcode-selfdev-types/src/lib.rs`: `SourceState` (`short_hash`/`full_hash`/`dirty`/`fingerprint`/`version_label`/`changed_paths`), `PublishedBuild` (`version`/`source_fingerprint`/`versioned_path`/`current_link`/`launcher_link`/`previous_current_version`), `PendingActivation` (`session_id`/`new_version`/`previous_current_version`/`previous_shared_server_version`/`source_fingerprint`), `BuildInfo`, `DevBinarySourceMetadata`, `CanaryStatus`. Notably **unchanged** fork-vs-base and upstream-vs-base (numstat `none`/`none`): the *data model* of identity is stable; only the *resolution policy* diverged.
3. **Reload-target selection** — `crates/jcode-build-support/src/paths.rs:528-720`: `client_update_candidate`, `shared_server_update_candidate`, `preferred_reload_candidate`, `nix_managed_launcher_override`/`nix_managed_override_target`, `version_matches_installed_channel`, `shared_server_channel_is_current_enough`. This is the seam's behavioral core.
4. **Pending/published activation lifecycle** — `crates/jcode-build-support/src/lib.rs:66,148-205,684-730`: `set/clear/complete/rollback_pending_activation_for_session`, `PublishedBuild` symlink publication (`current`/`launcher` links).
5. **Reload authority decision** — `crates/jcode-app-core/src/server/client_session.rs:706-740` (`handle_reload`, non-forced gate), `crates/jcode-app-core/src/server/util.rs:365+` (`server_has_newer_binary`, directional mtime-only check), `crates/jcode-app-core/src/server/reload.rs:83-153` (`ReloadSignal` with `prefer_selfdev_binary`).

## Every known source of runtime identity (the "four-way" claim, verified)

RESPONSIBILITIES.md invariant #1 requires manifest/launcher, daemon reload state, R03A subscribe fields, and R04 restart snapshots to describe the same identity. I located all four:

1. **Manifest/launcher/channel state** — `paths.rs` candidate resolution + `lib.rs` publication symlinks (`current_link`, `launcher_link`, `shared-server`/`stable` channels). Owner: R01.
2. **Daemon reload signal** — `server/reload.rs:83,153` `ReloadSignal { request_id, hash, triggering_session, prefer_selfdev_binary }`; `prefer_selfdev_binary` derived from `agent.is_canary()` at `client_session.rs:740-745`.
3. **Wire subscribe identity (carried, R03A)** — `crates/jcode-protocol/src/wire.rs:207,232-235` `Subscribe { selfdev: Option<bool>, protocol_version: Option<u32>, build_hash: Option<String> }`. `build_hash` = `jcode_build_meta::GIT_HASH`; comments explicitly cite `SELFDEV_NIX_DAEMON_DIVERGENCE.md` G1/G3. **R01 owns the meaning; R03A carries it.** Confirmed the field is a carriage vehicle, not a competing source.
4. **Restart snapshot (R04)** — `crates/jcode-session-types/src/lib.rs:195,201` `RestartSnapshot { jcode_version: String, is_selfdev: bool, ... }`. Persisted for crash/reload recovery.

This confirms the Coordinator's four-way consistency risk is real and correctly scoped: identity is written/read at four sites, and R01 is the intended arbiter of meaning. I did **not** exhaustively prove all four always agree at runtime (see gaps).

## Fork/upstream divergence (symbol-level, reproduced)

`git diff --numstat base..{fork,upstream}` on R01 paths:

| Path | fork | upstream | Read |
|---|---|---|---|
| `jcode-build-support/src/paths.rs` | **333+/1-** | none | **Reload-target selection is entirely fork-authored.** Zero upstream change. |
| `jcode-build-support/src/lib.rs` | 25+/5- | 2+/0- | fork-dominant (pending-activation lifecycle) |
| `jcode-build-meta/src/lib.rs` | 7+/0- | none | fork-only (`BUILD_SOURCE_DIR` etc.) |
| `jcode-selfdev-types/src/lib.rs` | none | none | identity data model stable both sides |
| `jcode-protocol/src/wire.rs` | 278+/70- | 3+/4- | subscribe identity fields fork-authored (R03A carriage) |
| `server/client_session.rs` | 139+/93- | 101+/49- | two-sided; reload gate is fork logic |

The PRESCREEN R01 aggregate (`PRESCREEN.md:72`) is 29/17/17 paths with "every upstream-matched path also changed in the fork; reload/runtime symbols diverged on both sides." My symbol-level read refines this: the **identity data model is shared/stable**, while the **resolution and reload-authority policy is overwhelmingly fork-only** (`paths.rs` 333+ vs 0 upstream). This is the decisive finding — there is no meaningful upstream reload-authority behavior to reconcile against; upstream simply lacks it.

Note: `server/util.rs` and `server/reload.rs` returned numstat `none` against base despite containing fork-authored logic (issues #291/#277 comments). This means the files are either unchanged at those exact paths since base or were introduced by an ancestor of the merge base; I did not fully resolve this (gap), but it does not affect the disposition since the live code clearly implements fork-only guardrails.

## Stale-daemon incident (PRESCREEN.md:118) vs current code

Incident (`bug-server-reload-stale-daemon-version-check.md`, hash `80012e2c…`, 2026-07-14): Nix binary, selfdev current build, stable channel, and live daemon diverged; reload reported "already newest" while the daemon still mapped an old executable; forced reload changed the mapped build while preserving sessions.

The current fork code encodes **two independent defenses** that directly target this failure:

1. **Directional, mtime-based update detection** (`util.rs:365-390` `server_has_newer_binary`): explicitly refuses to treat "my version differs from channel markers" as "I am outdated" (the exact conflation that caused #291 downgrade and the #277 reload loop). It only reports an update when a candidate binary is *strictly newer by mtime*, excludes reloading into itself, and treats uncertainty as "no update." This is a deliberate, well-reasoned fix.
2. **Client-authoritative staleness override** (`server_events.rs:60-142` `should_defer_history_for_runtime_identity_with_allow`): a client that independently measures the server's release as strictly older **defers and repairs the channel before reloading**, and this wins even over the daemon's own `server_has_update: Some(false)` self-report — precisely the "stale daemon says it's newest" symptom. Unit tests assert `Some(false)+client_detected_stale -> defer` (`server_events.rs` `client_detected_older_server_always_defers`).
3. **Non-forced reload no-op is explicit, forced reload still switches** (`client_session.rs:719-736`): matches the incident's observed "forced reload changed the mapped build while preserving sessions."

The nix-managed override (`paths.rs:545-561`) is the structural root fix: under external (nix) management, non-selfdev callers resolve straight to the launcher and **ignore the self-managed `builds/` shadow**, which is what let a stale self-dev build capture the running server ("self-certifying-channel version-drift incident", `paths.rs:531-544`).

## Tests run (isolated worktree, read-only)

`bash scripts/dev_cargo.sh test -p jcode-build-support --lib` -> **45 passed, 0 failed**. Incident-relevant passing cases:
- `update_leaves_daemon_reload_target_stale_when_shared_server_pinned_to_selfdev`
- `selfdev_reload_target_diverges_from_update_probe_when_shared_server_pinned`
- `update_advances_daemon_reload_target_when_shared_server_tracks_stable`
- `normal_shared_server_candidate_repairs_stale_shared_channel_to_stable`
- `repair_never_downgrades_when_stable_is_older`, `repair_preserves_fresher_selfdev_pin`
- `test_client_update_candidate_prefers_dev_binary_for_selfdev`, `pending_activation_can_complete_and_roll_back`

These are exactly the divergence/stale-pin/repair invariants the incident stresses, and they pass at the reviewed tree.

`should_defer_history_for_runtime_identity_with_allow` has inline unit tests (`server_events.rs` runtime_identity_tests) asserting the client-authoritative override. I attempted `jcode-app-core` `client_session_tests::reload` but it was blocked by a concurrent build-directory lock and I declined to expand the budget fighting it (narrow-rather-than-expand). The build-support suite already covers the seam's decisive invariants.

## Authority challenge (both sides)

- **Upstream authority: rejected for this seam.** Upstream has no reload-target selection (`paths.rs` upstream-diff empty). There is no upstream behavior to import; adopting upstream here would delete the incident fixes. Upstream's only overlap is shared identity value types and general wire/session churn, which are R03A/R04 concerns.
- **Fork authority: accepted with scrutiny.** The fork logic is incident-driven, heavily commented with issue provenance (#277/#291/#295/#405), and defended by passing adversarial tests. The one legitimate worry — mtime-based ordering is fragile (clock skew, in-place rebuild, `(deleted)` marker) — is explicitly acknowledged and mitigated in `util.rs:379-390` (base-semver cannot order dev builds, so mtime is the only directional signal; `(deleted)` suffix stripped; unreadable mtime = "no update"). This is a reasoned, conservative choice, not an oversight.

## Recommended disposition

**Keep fork R01 as authoritative; no upstream reconciliation required for reload-target selection or build identity.** The identity data model (`selfdev-types`) is already shared and stable. Treat R01 as a settled full seam whose invariants are (a) single-source compile-time identity, (b) directional-only update detection, (c) client-authoritative staleness repair, (d) nix-managed shadow bypass, (e) forced reload may switch build while preserving sessions. The disposition is **evidence-supported**, not merely plausible: the incident is reproduced as passing guard tests and the upstream side is empty.

## Cheapest decisive tests (for pilot / future gate)

1. Re-run `jcode-build-support --lib` (45 tests, ~0.6s) — already green here; is the fastest regression tripwire for reload-target selection.
2. Run `jcode-app-core` `server_events` runtime_identity_tests + `client_session_tests::reload` in a clean build (blocked here by lock) to cover the client-authoritative override end-to-end.
3. A non-mutating identity-consistency assertion: in a disposable daemon, assert `Subscribe.build_hash == jcode_build_meta::GIT_HASH`, `RestartSnapshot.jcode_version == VERSION`, and `ReloadSignal.prefer_selfdev_binary == agent.is_canary()` all read from the same source. (Not run here.)

## Pilot prerequisites (R01's contribution)

- Deterministic build/source identity for both fork and upstream pilot binaries, with a non-mutating check that the selected executable is the one reported/launched (`BUILD_SOURCE_DIR`/`GIT_HASH`).
- No live user daemon: the pilot must use a disposable daemon so the nix-managed override and mtime ordering behave as designed.
- If the pilot exercises reload/resume, R04 (restart snapshot) and R03A (subscribe carriage) must be co-validated — they consume R01 identity but must not re-derive it.

## Confidence and explicit gaps

- Confidence high: reload-target selection is fork-only; identity model is shared/stable; incident is encoded as passing invariants.
- Not fully resolved: `server/util.rs` and `server/reload.rs` returned empty numstat against base despite containing fork guardrail logic — I did not determine whether they predate the merge base or moved paths. Does not change the disposition but is an open provenance thread.
- Not proven: exhaustive runtime agreement of all four identity sources under a live reload (I read the four sites statically; the end-to-end reload test was build-lock-blocked).
- Not checked: desktop reload path (`desktop_reload.rs:797,820` `binary_is_newer_than`) — R08D scope, only noted as a second consumer of "newer binary" semantics that could re-derive identity independently.
- No live daemon, credentials, network, or destructive ops were used. No repo modifications.
