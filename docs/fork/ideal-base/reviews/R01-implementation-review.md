# R01 implementation review (independent adversarial)

- Reviewer model: claude-opus-4-8
- Mode: independent adversarial reviewer, read-only except this report file.
- Repo: `/Users/jrudnik/labs/jcode`, branch `main`.
- Implementation commits reviewed: `7387597e1`, `418c92935`, `d7807e4ca`, `c5ab2c9d5`, `4ad36e517`.
- Docs read: `terra_reload_version_report.md`, `terra_session_death_report.md`, `evidence/R01/IMPLEMENTATION.md`.

## Method

I read each commit diff (`git show`), read the resulting current sources, and re-ran
the stated test commands myself with `scripts/dev_cargo.sh`. I then attacked the
five adversarial vectors (a)-(e) directly against the code.

## Acceptance gate verification

All five gates were re-run / re-derived independently.

### Gate 5 (test suites) — VERIFIED PASS

Re-ran locally, not trusting the IMPLEMENTATION.md numbers:

```
scripts/dev_cargo.sh test -p jcode-build-support
  => ok. 50 passed; 0 failed; 0 ignored   (incl. both atomic-publish gates)
scripts/dev_cargo.sh test -p jcode-app-core
  => ok. 1133 passed; 0 failed; 23 ignored
```

Named regressions re-run in isolation, all `ok`:

- `server::util::target_resolution_tests::target_resolution_refuses_forced_stale_shared_server_when_newer_dev_exists`
- `server::util::target_resolution_tests::target_resolution_allows_forced_shared_server_when_it_is_freshest`
- `server::reload_recovery::tests::session_alias_roundtrips_and_follows_resume_chain`
- `server::reload::r01_tests::recovery_intents_include_attached_non_headless_sessions`
- `server::debug_command_exec::tests::resolve_debug_session_resolves_reconnect_alias_to_live_agent`
- `server::debug_command_exec::tests::resolve_debug_session_unknown_id_error_names_alias_and_active_sessions`

I also independently `cargo check`-ed the top-level `jcode` bin (the `commands.rs`
diff in `d7807e4ca` is not covered by either tested crate): clean, `Finished`.

### Gate 1 (concurrent source truncation cannot corrupt a published artifact) — PASS

`copy_binary_to_staging_path` (`crates/jcode-build-support/src/lib.rs:322-393`) opens
a per-attempt unique staged file with `create_new(true)` (pid + nanos + attempt),
streams `source` into it, `set_permissions_executable`, `sync_all`, and rejects a
zero-byte result before returning. `publish_staged_binary` (`lib.rs:410-426`) then
`fs::rename`s the fully-formed staged file over `versions/<v>/jcode`. A source
truncation after staging cannot reach the published copy because the published copy
is a distinct, already-synced file. The regression test
`atomic_publish_tests::concurrent_source_truncation_between_stage_and_rename_preserves_published_copy`
asserts the published bytes equal the pre-truncation original and that smoke runs the
`.jcode-publish-*` staged path, never the source or final path. Two concurrent
publishers of the *same* version each stage a unique file and each rename a *complete,
smoke-passed* copy over `dest`; last-writer-wins is benign because both are valid.
`remove_dir` (not `remove_dir_all`) on the failure path (`lib.rs:314-316`) cannot
delete a sibling's already-published `jcode`. Solid.

### Gate 3 (failed smoke leaves no versions/<v> entry) — PASS

On any pre-publish error `install_binary_at_version_in_builds_dir` removes the staged
file and, when `dest` does not exist, removes the (now-empty) version dir
(`lib.rs:305-319`). `atomic_publish_tests::failed_smoke_test_leaves_no_version_entry`
asserts the version dir is absent after a failing smoke. No usable
`versions/<v>/jcode` can survive a failed smoke. Matches the incident's root cause
(the zero-byte hard-linked `a87c5f271/jcode`) being eliminated.

### Gate 2 (forced reload refuses / publishes, never silently returns old payload while a strictly newer valid dev binary exists) — PASS for the forced path

`handle_reload` computes `resolve_reload_target(prefer_selfdev_binary, force)` up
front and, for `force == true`, returns a client-visible `Error` refusal before any
shutdown when `forced_stale_shared_server_refusal` fires
(`client_session.rs:734-754`, `util.rs:128-133`, `util.rs:460-485`). The refusal
names both paths, payloads, and mtimes. `target_resolution_refuses_forced_stale_shared_server_when_newer_dev_exists`
proves the predicate. This satisfies the letter of gate 2 for the forced reload.
(But see BLOCKING-1: the *same* refusal is also applied unconditionally at the exec
stage, which is where it becomes dangerous.)

### Gate 4 (alias resolves to resumed agent; stale-id error names resolved target) — PASS

`resolve_debug_session` resolves an explicit or fallback id through
`resolve_session_alias` and returns the live resumed agent
(`debug_command_exec.rs:71-103`). The stale-id error names the requested id, the
resolved alias target, and the active-session list (`:88-100`). Both named tests
pass. Alias chains are bounded (16 hops) with a `seen`-set cycle guard and a
self-alias / empty-target break (`reload_recovery.rs:259-282`); `persist_session_alias`
refuses self-aliases (`:239-241`).

## BLOCKING findings

### BLOCKING-1 — The exec-stage refusal can kill a self-dev daemon (no replacement) during a routine non-forced `jcode server reload`, re-introducing the stranded-session class R01 exists to fix

The refusal is applied in **two** places with divergent `force`:

1. Preflight, with the request's real `force`:
   `client_session.rs:734` `resolve_reload_target(prefer_selfdev_binary, force)`.
   For a non-force request `force == false`, and `forced_stale_shared_server_refusal`
   is only computed `if force` (`util.rs:128`). So a non-force preflight **never
   refuses**; it proceeds whenever a strictly-newer exec candidate exists
   (`client_session.rs:762`).

2. Exec stage, with `force` **hardcoded to true**:
   `reload_exec_target` (`util.rs:86-95`) calls
   `resolve_reload_target(is_selfdev_session, true)` and returns `None` when the
   refusal fires. `ReloadSignal` (`reload_state.rs:416-423`) carries no `force`
   field, so the reload worker cannot know the original request was non-force.
   `reload.rs:178` turns that `None` into the `else` branch
   (`reload.rs:223-231`, "no reloadable binary found") and then
   `super::shutdown::coordinator().reload_exec_failed().await` (`reload.rs:236`),
   which drains and **exits the process (exit 42) with no replacement execed**.

**Reproduction (composition of already-passing facts + reachability trace):**

State: self-dev daemon currently running build `v0`; `shared-server` pinned to a
promoted self-dev build `v1` (newer than `v0`); an unpromoted newer
`target/selfdev/jcode` = `v2` present on disk (normal after a fresh rebuild).
Invoke the default `jcode server reload` (no `--force`).

- Preflight `resolve_reload_target(prefer=true, force=false)`: `refused = None`.
  `has_strictly_newer_candidate_than_current()` compares `current=v0` against the
  exec candidates `server_update_candidate(true)=shared-server v1` and
  `server_update_candidate(false)=stable`. `v1 > v0` ⇒ true ⇒ preflight does **not**
  skip and **does not** refuse; it sends the reload signal and drains sessions.
- Exec `reload_exec_target(true)` = `resolve_reload_target(true, force=true)`.
  `collect_reload_target_candidates` always adds the raw `dev` candidate
  `target/selfdev/jcode = v2` (`util.rs:` dev branch, gated only by
  `get_repo_dir()`/`find_dev_binary`, both present on a self-dev daemon).
  `forced_stale_shared_server_refusal` finds `shared-server v1` is strictly older
  than `dev v2` and returns `Some(...)` — exactly the case proven by
  `target_resolution_refuses_forced_stale_shared_server_when_newer_dev_exists`.
  `reload_exec_target` returns `None`. Note the aggravating detail: the `dev`
  candidate is inserted with `exec_candidate=false` (`util.rs:358-363`), so it is
  never eligible to be chosen as the exec target (the picker and
  `has_strictly_newer_candidate_than_current` both filter on `exec_candidate`,
  `util.rs:117-121,228-234`). Yet `forced_stale_shared_server_refusal` scans the
  full candidate vector (`util.rs:128-130` passes `&candidates`), so a binary that
  is explicitly *not* an exec candidate still vetoes the reload onto the approved
  one.
- `reload.rs:223-236`: daemon writes `Failed` and exits with no replacement.

The most damning detail: the `force`-gated refusal short-circuits
`resolve_reload_target` *before* its own anti-stranding fallback can run. When
`refused = Some(...)` (`util.rs:128-133`) the entire `if refused.is_none()` block
(`util.rs:135`) is skipped, so `chosen` stays `None`. That skipped block is exactly
where `resolve_reload_target` implements "a live downgrade beats a dead server with
no replacement" — it falls back to the candidate (or current exe) rather than
returning nothing (`util.rs:161-171`, `DowngradeBlockedUseCurrent`). The refusal
thus defeats the very stranding-prevention the resolver was written to provide, and
hands `reload_exec_target` a `None` that becomes a process exit. The prior
`newest_reload_candidate` selection (now `#[cfg(test)]`-only, `util.rs:502-521`)
execed the approved `shared-server v1` here — an **upgrade** from `v0`, daemon stays
alive, sessions preserved. The new code instead exits. This is a strict
availability regression on the default reload command, and it re-creates the
"daemon severs all client sockets with no live successor" failure mode that
`terra_session_death_report.md` and R01 are meant to prevent. The unpromoted
transient `target/selfdev/jcode` is precisely the "transient local self-dev binary"
that the incident report's fix proposal #1 says must **not** influence the shared
daemon; here its mere presence on disk vetoes a reload onto the approved target and
kills the daemon.

Why it is untested: `c5ab2c9d5` disabled the preflight refusal under `cfg!(test)`
(`client_session.rs:741` `if !cfg!(test) && ...`), and no test exercises
`reload_exec_target` returning `None` or the `reload.rs` exec-fail path with a
refusal. The refusal tests only cover the pure `forced_stale_shared_server_refusal`
function, not the exec wiring.

Blast radius: self-dev daemons (release daemons have no repo, so `get_repo_dir()`
returns `None` and the `dev` candidate is absent — release users are not affected by
this specific dev-based veto; see focus (e) below). Recovery is partial: a fresh
daemon spawns on the next client connection using the approved binary and sessions
are checkpointed, but the in-flight reload returns an error and all live TUIs are
dropped mid-reload.

Suggested fix (any one):
- Thread the original `force` into `ReloadSignal` and have `reload_exec_target` pass
  it to `resolve_reload_target`, so a non-force reload never refuses at exec.
- At the exec stage, on refusal fall back to the approved `shared-server`/current
  exec candidate (keep the daemon alive) instead of returning `None` → exit.
- Restrict `forced_stale_shared_server_refusal` to compare `shared-server` only
  against *promotable/approved* candidates, never against the raw unpromoted
  `target/selfdev/jcode`.

## Non-blocking findings

### N1 — Session-alias files are never garbage-collected (unbounded growth)

`persist_session_alias` writes one `~/.jcode/session-aliases/<id>.json` per resumed
source/client session (`reload_recovery.rs:233-256`). `collect_garbage`
(`reload_recovery.rs:133-` ) scans only `recovery_dir()`, never `alias_dir()`.
Every reconnect/resume across the process lifetime leaves a permanent small JSON
file. Not corrupting and each is tiny, but it is a slow unbounded leak and stale
alias files persist indefinitely (they can outlive their target agent; the stale-id
error path (gate 4) handles that gracefully, so correctness holds). Recommend adding
alias files to the GC sweep with an age/TTL bound. (Focus (c): chains are bounded
and cycle-guarded, so no infinite loop or unbounded chain — only unbounded *file
accumulation*.)

### N2 — Expanded recovery eligibility is correctly scoped, but note the wait set is unchanged (good)

`persist_reload_recovery_intents` now includes `!is_headless && !event_txs.is_empty()`
members regardless of `running` (`reload.rs:271-273`). The triggering CLI client is
**not** wrongly rehydrated: it is still assigned `was_interrupted=false` and, absent
a `ReloadContext`, `recovery_directive_for_session` returns `None`, so it is skipped
(`reload.rs:291-318`; `selfdev/reload.rs:127-149`). The graceful-shutdown wait set
still filters on `status == "running"` only (`reload.rs:393-400`), so the broader
eligibility does not expand the wait set or risk a reload hang. `r01_tests` confirms
attached-ready peers get intents while detached-ready and headless-attached do not.
Focus (d) is satisfied. One product note: a truly-idle attached TUI now receives the
generic "interrupted" continuation, which is intended per the incident proposal #3
but is slightly misleading text for a session that was not generating.

### N3 — Release-channel (non-selfdev) repair path preserved (focus e)

`d7807e4ca` removed the client-side `repair_stale_shared_server_channel()` call from
`run_server_reload_command`, but the repair is still invoked on the release update
paths that need it: `src/cli/hot_exec.rs:404-428` + `:184-186`/`:356-358`, and
`crates/jcode-app-core/src/update.rs:1172,1187-1206`. The refusal logic that must
keep working for release users (`lib.rs` `repair_stale_shared_server_channel`,
`is_release_channel_marker`, `shared_server_binary_is_strictly_older_than`,
now ~`lib.rs:1181-1269`) is unchanged and its tests pass
(`repair_*` in `build-support`, 50/50). Removing the redundant repair from the manual
`server reload` command is acceptable because release daemons resolve `shared-server`/
`stable` and are not subject to the dev-candidate veto of BLOCKING-1. No release-user
regression found.

### N4 — `sanitize_session_id` collision (theoretical)

Distinct ids that sanitize to the same string (e.g. `a/b` vs `a_b`) would share an
alias file. Session ids are alphanumeric + timestamp + random, so collision is
improbable; noting for completeness, not blocking.

## TOCTOU assessment (focus a) — clean

The stage→smoke→rename pipeline uses unique per-attempt staged names, `create_new`
exclusivity, `sync_all` before rename, a zero-byte guard, and an atomic rename; a
concurrent same-version publisher can only replace the final path with another
fully-validated copy. A torn/partial read of a concurrently-rebuilding source is
caught by the staged smoke test (or the zero-byte guard) and never reaches
`versions/<v>/jcode`. No interleaving corrupts a published artifact.

## Verdict rationale

Gates 1, 3, 4 are cleanly satisfied. Gate 2's forced-path refusal is satisfied and
Gate 5's suites pass (independently re-run). However, the refusal was wired a second
time at the exec stage with a hardcoded `force=true` that is unreachable-by-design
from the tested preflight but **is** reachable from a routine non-forced
`jcode server reload` on a self-dev daemon, where it converts an approved
session-preserving reload into a daemon exit with no replacement — the exact
stranded-session class this node exists to eliminate, and it is entirely untested.
For a reload-robustness repair node, that is a blocking regression.

VERDICT: FAIL
