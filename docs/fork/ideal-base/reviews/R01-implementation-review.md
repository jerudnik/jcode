# R01 implementation review (independent adversarial)

**Reviewer model:** claude-opus-4-8
**Mode:** read-only source + artifact investigation, plus independent rerun of the named acceptance gates. No implementation files changed.
**Repository:** `/Users/jrudnik/labs/jcode`, branch `main`.
**Implementation commits reviewed:** `7387597e1`, `418c92935`, `d7807e4ca`, `c5ab2c9d5`, `4ad36e517`.
**Continues:** stallion's interrupted review (session lost server-side). Its verified results are re-checked below and adopted where independently reproduced.

---

## Verdict summary

**FAIL** — one blocking regression in the reload feature's primary path (finding F2), plus one non-blocking Medium finding (F1, alias GC gap). The atomic-publish work (finding focus a/b) and recovery-eligibility broadening (focus d) are sound.

---

## Acceptance gates (independently rerun)

| Gate | Result |
|---|---|
| `jcode-build-support` suite | **PASS** 50 passed / 0 failed (incl. both atomic-publish gates: `concurrent_source_truncation_between_stage_and_rename_preserves_published_copy`, `failed_smoke_test_leaves_no_version_entry`) |
| `target_resolution_tests::*` (both) | **PASS** 2/2 |
| `recovery_intents_include_attached_non_headless_sessions` | **PASS** |
| `session_alias_roundtrips_and_follows_resume_chain` | **PASS** |
| `resolve_debug_session*` (6 matched) | **PASS** 6/6 incl. `resolve_debug_session_resolves_reconnect_alias_to_live_agent`, `resolve_debug_session_unknown_id_error_names_alias_and_active_sessions` |

The full 1133-test app-core run was not re-executed end-to-end here (time budget); the focused gates and every named regression were independently reproduced green. Stallion's 1133/0 claim is consistent with these results and the unchanged surrounding code.

---

## BLOCKER F2 — exec-stage reload refusal uses hardcoded `force=true`, so a legitimate NON-force `jcode server reload` can drop live sessions and `exit(42)` instead of upgrading

**Severity: Blocking.** This is the suspected finding 2, now resolved as a *real* regression, not intended fail-closed behavior.

### The asymmetry (root cause)

- Preflight (`handle_reload`) resolves the target with the **real** force flag:
  `crates/jcode-app-core/src/server/client_session.rs:734`
  `let target_resolution = super::util::resolve_reload_target(prefer_selfdev_binary, force);`
- The exec stage **hardcodes `force=true`** regardless of the request:
  `crates/jcode-app-core/src/server/util.rs:86-87`
  `pub(crate) fn reload_exec_target(is_selfdev_session: bool) -> Option<(PathBuf, &'static str)> { let resolution = resolve_reload_target(is_selfdev_session, true); ... }`
- `ReloadSignal` (`crates/jcode-app-core/src/server/reload_state.rs:417-424`) does **not** carry `force`, so the reload worker (`reload.rs:176-178`) cannot pass the request's force into `reload_exec_target`.
- The refusal is gated on force inside the resolver:
  `crates/jcode-app-core/src/server/util.rs:128-133` — `if force && let Some(reason) = forced_stale_shared_server_refusal(&candidates) { refused = Some(reason); }`
  and when `refused.is_some()`, the resolver **skips computing `chosen`** entirely (`util.rs:135` `if refused.is_none() { ... }`), so `reload_exec_target` returns `None` (`util.rs:89-94`).
- `None` at the exec stage is now **fatal**: `reload.rs:223-231` writes `ReloadPhase::Failed("no reloadable binary found")` and `reload.rs:236` calls `super::shutdown::coordinator().reload_exec_failed().await` → historic bare `exit(42)` with no replacement process.

The pre-`d7807e4ca` `reload_exec_target` **never returned `None`** on a downgrade/refusal — it fell back to re-execing the current binary (see `git show d7807e4ca^:.../util.rs`, all match arms return `Some(...)`). The new resolver introduces a `None` path that kills the daemon.

Additionally, the client-side repair that previously made this exact scenario succeed was **removed** from the command in `d7807e4ca`:
`src/cli/commands.rs:2177` now calls `client.reload_with_force(force)` directly; the prior `crate::build::repair_stale_shared_server_channel()` call (which repointed `shared-server -> stable` before reload) is gone. The repair still runs in the auto-reload (`server_events.rs:1560`), `/update` (`update.rs:1188`), and `hot_exec.rs:405` paths, but **not** in the explicit `jcode server reload` command — the "no-op /update, long-lived old daemon" case the repair function was written for (`lib.rs:1164-1180`).

### Why the non-force path reaches the fatal exec refusal

For `force == false`, the preflight refusal check at `client_session.rs:741-754` never fires, because the refusal is force-gated (`util.rs:128`) and preflight passed `force = false`. Whether the reload proceeds is then decided solely by the non-force skip gate at `client_session.rs:762`:
`if !force && !target_resolution.has_strictly_newer_candidate_than_current() { skip }`.

`has_strictly_newer_candidate_than_current` (`util.rs:228-238`) considers only the `exec_candidate` entries — the `preferred`/`alternate` results of `server_update_candidate` (`util.rs:333-339`). `forced_stale_shared_server_refusal` (`util.rs:460-484`) considers **all** candidates and keys off the `shared-server` label. These two decisions use different candidate sets and can disagree.

### Reproduction (release user, no repo required)

Field state — the canonical "current client, stale server" scenario from the incident report and the reason `repair_stale_shared_server_channel` exists:

- Daemon process running old payload **X** (`current_exe` payload mtime 100).
- `stable` channel advanced to newer release **Y** (mtime 300) — e.g. a newer release shipped.
- `shared-server` channel still pinned to **X** (never advanced; `shared-server-version != stable`, `!= current`).
- Client `current-version = Y` (client on newest, so `/update` is a no-op and never re-runs the install/advance path).

Run `jcode server reload` (default, **non-force**):

1. `server_update_candidate(false)` returns `stable` **Y**, because `shared_server_channel_is_current_enough()` is false (`paths.rs:625-651`: `shared(X) != stable(Y)` and `!= current(Y)`). So an `exec_candidate` with payload Y (mtime 300) exists.
2. Skip gate `has_strictly_newer_candidate_than_current()` = **true** (Y newer than current X) → reload **proceeds** past `client_session.rs:762`.
3. `mark_remote_reload_started`, `Reloading` fan-out to live sessions, `send_reload_signal`, `Done` to the CLI (`client_session.rs:781-824`).
4. Reload worker: `persist_reload_recovery_intents` + `graceful_shutdown_sessions` (live headless/swarm sessions are shut down), then `reload_exec_target(prefers_selfdev)` (`reload.rs:178`).
5. `reload_exec_target` → `resolve_reload_target(_, true)`. Now `forced_stale_shared_server_refusal` runs: `shared-server` **X** exists and is the min-mtime `shared-server`-labeled candidate; `stable` **Y** is a different-payload candidate strictly newer → **refuses** → `None`.
6. `reload.rs:223-236` → `Failed` → `reload_exec_failed()` → **`exit(42)` with no replacement**.

### Impact

- The daemon performs graceful shutdown of live headless/swarm sessions and then **dies** with no handoff — the exact outcome `jcode server reload` was built (issue #291) to avoid ("instead of killing the daemon and dropping live headless/swarm sessions, we ask it to hand off").
- The CLI `run_server_reload_command` observes `Reloading` then `Done`, so it enters `await_reload_handoff(30s)` (`commands.rs:2230`), times out, and reports the misleading "reload requested; the new server is still coming up." (`commands.rs:2238`) — not a clean skip and not a genuine failure surfaced to the user.
- Not a strict infinite refuse-loop: the next client spawn launches a fresh daemon on `current = Y` via the launcher, and recovery intents may restore sessions. But this is a self-inflicted daemon kill on the graceful path, in the precise state the feature targets, and it is a regression from a path that previously **worked** (old code: client repair `shared-server -> Y`, then exec into Y).

### Note on the refusal's own intent

Even by its stated intent ("refuse instead of silently re-execing the *stale* payload", IMPLEMENTATION.md §2), the refusal is over-broad: `pick_newest_target_candidate` would have selected `stable` **Y** (the newest exec candidate), **not** the stale `shared-server` X. The daemon was never going to re-exec the stale payload here; the refusal blocks a legitimate upgrade to Y and returns `None` instead of falling back to the already-computed newest good target.

### Suggested remediation (not implemented; for the owner)

Any one of:
- Thread `force` through `ReloadSignal` so the exec stage uses the request's real force (symmetry with preflight).
- When `forced_stale_shared_server_refusal` fires, fall back to the computed `chosen` (newest valid exec candidate) rather than returning `None`, so the daemon never dies with a valid newer target in hand.
- Keep/restore the client-side `repair_stale_shared_server_channel()` in `run_server_reload_command` so the non-force path advances `shared-server -> stable` before signaling (the pre-`d7807e4ca` behavior).
- Or apply the refusal at preflight for non-force too and convert it into a clean `skip` (Done, daemon lives), never reaching graceful shutdown + exec death.

---

## F1 — session-alias files have no garbage collection (confirmed; Medium, non-blocking)

Adopting and independently confirming stallion's finding 1.

- Alias records are written durably and per-source-session, overwriting in place:
  `reload_recovery.rs:233-257` (`persist_session_alias`, `crate::storage::write_json` to `alias_path_for_session`), directory `session-aliases/` (`reload_recovery.rs:79-81`).
- The GC sweep only reads `recovery_dir()` (`reload-recovery/`): `collect_garbage_at` at `reload_recovery.rs:137-138` iterates `recovery_dir()` exclusively; it never touches `alias_dir()`. Grep for alias cleanup/prune/expire in that module returns nothing.
- Write call sites: resume binding (`client_lifecycle.rs:303`) and debug reconnect (`debug_command_exec.rs:734`, `:768`). Files are keyed by (sanitized) source session id and overwritten, so growth is bounded by the number of **distinct** source/client session ids seen over the daemon's lifetime, not by reload count. Still unbounded over time with no TTL and no `PENDING_RECORD_MAX_AGE` (7-day) sweep equivalent.

**Severity: Medium.** No correctness impact (alias resolution is bounded to 16 hops with cycle detection, `reload_recovery.rs:259-281`), but a long-lived daemon accrues `session-aliases/*.json` indefinitely. The fix is small: extend `collect_garbage_at` to sweep `alias_dir()` by mtime against `PENDING_RECORD_MAX_AGE` (mirroring the orphan-backup path). Not blocking.

---

## Focus (a) — atomic-publish TOCTOU (two concurrent publishers, same version): PASS

`install_binary_at_version_in_builds_dir` (`lib.rs:282-320`) with `copy_binary_to_staging_path` (`lib.rs:322-393`) and `publish_staged_binary` (`lib.rs:410-426`) is safe under concurrent publishers of the same version:

- Each publisher stages to a **unique** hidden temp path (`.jcode-publish-<pid>-<nanos>-<attempt>`, `lib.rs:395-408`) created with `create_new(true)` (`lib.rs:325-338`) — no staged-file collision; `AlreadyExists` retries a new name.
- Bytes are copied (not hard-linked), `sync_all`'d, and a **zero-byte staged file is rejected** (`lib.rs:360-369`) — this directly fixes the incident's zero-byte hard-link artifact.
- Smoke test runs on the **staged** path (`lib.rs:307`, `smoke_test_staged_binary_for_install`), then `std::fs::rename(staged, dest)` publishes atomically (`lib.rs:417`). The two staged copies are byte-identical (same source version); whichever renames last wins with an identical, valid binary. No zero-byte/partial file is ever visible at `versions/<v>/jcode`.
- Failure cleanup is race-safe (`lib.rs:312-317`): remove staged file, and remove the version dir only `if !dest.exists()`. If publisher A succeeded, B's failure leaves A's `dest` intact (B does not remove the dir). If both fail, `remove_dir` on a non-empty or already-removed dir is swallowed (`let _ =`); worst case a leftover empty dir. No corruption.

The two shipped gates (`concurrent_source_truncation_between_stage_and_rename_preserves_published_copy`, `failed_smoke_test_leaves_no_version_entry`) exercise the stage-vs-source-truncation race and the failed-smoke cleanup, and both pass. Sound.

---

## Focus (b) — `repair_stale_shared_server_channel` semantics unchanged: PASS

The only R01 commit touching `jcode-build-support/src/lib.rs` is the atomic-publish commit `7387597e1`, and it does **not** touch the repair region. Byte-for-byte diff of the function body pre-R01 (`7387597e1^`) vs current HEAD is **identical** (verified). Release-channel gating (`is_release_channel_marker`, `lib.rs:1202-1209`, `:1230-1237`) and the strictly-newer-by-payload-mtime guard (`shared_server_binary_is_strictly_older_than`, `lib.rs:1248-1269`) are unchanged. `repair_repoints_stale_shared_server_to_newer_stable` and the surrounding suite pass. No regression for release-channel users at this function. (Note: this is orthogonal to F2, which concerns the *removal of the caller* in `run_server_reload_command`, not the function itself.)

---

## Focus (d) — recovery eligibility (attachment-based) wrongly including the triggering ephemeral CLI session: PASS (with note)

The broadened eligibility filter (`reload.rs:271-273`): `member.status == "running" || (!member.is_headless && !member.event_txs.is_empty())`.

- The triggering session is always appended as a candidate (`reload.rs:278-284`), but a **directive** is only persisted when `recovery_directive_for_session` returns `Some` (`reload.rs:292-318`), which requires either a persisted `reload_ctx` or `was_interrupted` (`tool/selfdev/reload.rs:127-149`). For the triggering session, `was_interrupted = is_headless || !is_triggering = false`, so an ephemeral CLI-triggered session with no reload context yields **no directive** and is skipped — it does not pollute recovery.
- The shipped gate `recovery_intents_include_attached_non_headless_sessions` confirms: attached non-headless non-running → included; detached non-headless → excluded; headless-attached non-running → excluded. Independently reproduced green.
- Minor residual: any attached non-headless member is now a *candidate* regardless of status, so a genuinely attached interactive peer with a reload context is correctly recovered; an attached ephemeral session without context is filtered at the directive stage. No wrongful inclusion of the triggering ephemeral CLI session. Not a blocker.

---

## Conclusion

The atomic-publish repair (F1-incident fix), the reload-target diagnostics, resume-alias binding, and recovery broadening are well-built and independently pass their gates. However, the reload target commit `d7807e4ca` introduced a real regression: the exec stage refuses with a hardcoded `force=true` while `ReloadSignal` carries no force and the client-side channel repair was removed from `run_server_reload_command`. A default (non-force) `jcode server reload` in the stale-`shared-server`/newer-`stable` state — the exact scenario the feature targets — passes the non-force skip gate, performs graceful session shutdown, then hits the force-gated exec refusal, returns `None`, and `exit(42)`s the daemon with no handoff. This is a path that previously worked (client repair, then exec into stable) and now kills the daemon and its live sessions.

VERDICT: FAIL (superseded — see re-review below)

---

## Re-review of BLOCKING-1 fix

**Re-reviewer model:** claude-opus-4-8
**Mode:** read-only source + evidence investigation, plus independent rerun of the four named regression tests and both full suites. No implementation files changed.
**Fix commits reviewed:** `923bba4aa` (force propagated through `ReloadSignal` into `reload_exec_target`), `293384c53` (bounded session-alias GC), `d2d876ffc` (evidence). All three confirmed ancestors of `HEAD`.

### (1) Kill-chain from BLOCKING-1 (F2) is closed

Traced end to end on current `HEAD`:

- `client_session::handle_reload` preflight resolves with the **real** force (`client_session.rs:734`, `resolve_reload_target(prefer_selfdev_binary, force)`), then signals with the **same** force via `send_reload_signal_with_force(hash, triggering_session, prefer_selfdev_binary, force)` (`client_session.rs:811-816`). No longer calls the force-agnostic `send_reload_signal`.
- `ReloadSignal` now carries `force` (`reload_state.rs:421-422`, `#[cfg(not(test))] pub force: bool`) exposed via `ReloadSignal::force()` (`reload_state.rs:427-441`). Production build stores the field; the test build reads a per-request-id side table defaulting to `true`, so legacy fixtures that construct `ReloadSignal` without `force` keep their historical semantics.
- `reload::await_reload_signal` records `signal.force()` into the reload trace (`reload.rs:97`) and passes it to the exec stage: `reload_exec_target(prefers_selfdev, signal.force())` (`reload.rs:183`).
- `reload_exec_target(is_selfdev_session, force)` forwards to `resolve_reload_target(is_selfdev_session, force)` (`util.rs:88-92`) — **the hardcoded `true` is gone**.
- The refusal remains force-gated (`util.rs:149`, `if force && let Some(reason) = forced_stale_shared_server_refusal(&candidates)`). For a non-forced request (`force == false`), the refusal branch is never entered, so `refused` stays `None` and the resolver proceeds to compute `chosen` from the newest exec candidate (`util.rs:154-210`). The exact BLOCKING-1 scenario (stale `shared-server` X / newer `stable` Y, non-force) now resolves `chosen = (stable, "stable")` and execs, instead of returning `None`.

**Confirmed:** the only remaining hardcoded-`true` `resolve_reload_target` caller is `send_reload_signal`'s default (`reload_state.rs:542`), which fans out to the debug (`debug_command_exec.rs:646`), selfdev (`reload.rs:352`, `setup.rs:294`), and legacy paths — all pre-existing force-`true` semantics, none reachable from a non-force `jcode server reload`. Preflight and exec now consume the same `force`, so no request that passes preflight can be reinterpreted into an exec-stage refusal. Grep confirms the only exec-stage `reload_exec_target` call site (`reload.rs:183`) threads `signal.force()`. Kill-chain closed.

### (2) Named regression tests (rerun individually, all PASS)

| Test | Result |
|---|---|
| `server::reload_state::non_forced_reload_signal_retains_request_force` | **PASS** 1/1 |
| `server::util::target_resolution_tests::non_forced_stale_shared_server_with_newer_candidate_resolves_exec_target` | **PASS** 1/1 |
| `server::util::target_resolution_tests::target_resolution_refuses_forced_stale_shared_server_when_newer_dev_exists` | **PASS** 1/1 |
| `server::reload_recovery::tests::garbage_collection_sweeps_stale_session_aliases` | **PASS** 1/1 |

The exec-target test asserts `resolution.chosen_target() == Some((stable, "stable"))` in the non-force stale-shared/newer-stable state — the precise F2 outcome, now the tested contract. The forced-refusal test confirms the refusal is unchanged for `force == true`.

### (3) Alias GC cannot delete FRESH aliases or delivered-recovery records prematurely — confirmed

- The alias sweep (`collect_session_aliases_at`, `reload_recovery.rs:215-250`) removes a `*.json` **only** when `file_is_expired(&path, now)` is true, i.e. file mtime age `>= PENDING_RECORD_MAX_AGE` (7 days, `reload_recovery.rs:7`, `:119-125`). Fresh aliases are counted `retained` and kept. `persist_session_alias` writes via `storage::write_json` (`reload_recovery.rs:297-298`), which refreshes mtime on every re-persist, so an actively-resumed alias never ages out. The test proves a stale (mtime backdated past 7d) alias is removed while a fresh one and its resolution survive.
- Alias GC is orthogonal to recovery-record GC (`collect_recovery_records_at`, `reload_recovery.rs:151-213`), which is unchanged by the fix. Delivered records were already removed there by design (`status == Delivered`, `:189`); the alias sweep touches only `alias_dir()` (`session-aliases/`), never `recovery_dir()`, so it cannot delete delivered-recovery records at all. No premature deletion.
- One benign residual: `remove_record_files` also unlinks the `.bak` sibling. Alias writes do produce a `.bak` (via `write_json` on overwrite), so both primary and backup are cleaned together — correct, no orphaned `.bak`.

### (4) No new inconsistency introduced

- **Other `ReloadSignal` readers:** grep shows the only `force`/`force()` consumers are the trace (`reload.rs:97`), the log line (`reload.rs:88`), and the exec call (`reload.rs:183`). No other code branches on force.
- **Serialization:** `ReloadSignal` derives only `Clone, Debug` (`reload_state.rs:416`) — no serde — so the `#[cfg(not(test))]` field cannot break any wire/disk format. The trace serializes `signal.force()` into a hand-built `json!` value, not the struct.
- **Test-mode constructions** (`reload_tests.rs:53`, `:83`, `selfdev/tests.rs:398`, `:407`) omit the cfg-gated `force` field and compile cleanly; their `force()` defaults to `true`, preserving prior fixtures. The full suites confirm this compiles and passes.
- **Debug `reload` path:** unchanged. `debug_command_exec.rs:646` still calls `send_reload_signal(hash, None, false)` (prefer_selfdev = false), which routes through the force-`true` default — its historical exec semantics are preserved; the debug path sets `prefer_selfdev`/reload-info independently and is unaffected by the force threading.
- **selfdev/setup reload paths** (`selfdev/reload.rs:352`, `setup.rs:294`) use the runtime-identity / plain variants that also default force = `true`, exactly as before.
- **Reload traces/serialization:** additive only (new `"force"` key), no removed or renamed fields.
- **Minor nit (non-blocking):** the new unit test `non_forced_reload_signal_retains_request_force` (`reload_state.rs:1142-1159`) is placed just *after* the `#[cfg(test)] mod tests { ... }` closing brace (`:1141`) rather than inside it. It still compiles because `#[test]` items are only built under the test harness, and it passes, but it sits at module top level instead of within the tests module. Cosmetic; no functional impact.

### (5) Suites — both PASS

| Suite | Result |
|---|---|
| `scripts/dev_cargo.sh test -p jcode-build-support` | **PASS** 50 passed / 0 failed |
| `scripts/dev_cargo.sh test -p jcode-app-core` | **PASS** 1136 passed / 0 failed / 23 ignored |

Independently reproduced; consistent with FIX1.md's reported numbers.

### Re-review verdict

BLOCKING-1 (F2) is fully resolved: `force` is threaded symmetrically through `ReloadSignal` into `reload_exec_target`/`resolve_reload_target`, the exec stage no longer hardcodes `true`, and a non-forced `jcode server reload` in the stale-`shared-server`/newer-`stable` state now resolves the newer exec target rather than draining sessions and `exit(42)`ing. The F1 alias-GC gap is closed with a bounded 7-day mtime sweep that cannot evict fresh aliases or delivered records. No new blockers, no regressions in either suite. The single nit (test placement outside `mod tests`) is cosmetic.

RE-REVIEW VERDICT: PASS (R01 accepted)
