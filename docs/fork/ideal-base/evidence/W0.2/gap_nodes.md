# W0.2 gap nodes: proposed graph amendments from the source re-audit

Recorded: 2026-07-18 at commit `65175cff4`. Companion to `source_census.md`.
These are proposals for the coordinator (owner of WORK_GRAPH.json/STATE.json);
this node modified neither.

## GN-1: fix stale F06 owned path for mcp-serve (amendment, not a new node)

- Stale: F06 owns `src/cli/commands/**/*mcp*`, which matches zero files.
  Evidence: `find src/cli/commands -iname '*mcp*'` returns nothing; the
  directory contains only doctor.rs, menubar.rs, mobile_server.rs,
  provider_setup.rs, report_info.rs, restart_tests.rs, restart.rs.
- Current location: `src/cli/mcp_serve.rs:40` `run_mcp_serve_command` plus
  dispatch in `src/cli/dispatch.rs` and `src/cli/commands.rs` (both reference
  mcp_serve per ripgrep).
- Proposal: replace the glob with `src/cli/mcp_serve.rs` (and, if dispatch
  edits are needed, `src/cli/dispatch.rs`). Without this, F06's owner-PID
  self-liveness gate cannot be implemented inside its ownership boundary.

## GN-2: add jcode-selfdev-types to F09 owned paths (amendment)

- The `PendingActivation` struct that F09 must extend with reconciliation
  fields lives at `crates/jcode-selfdev-types/src/lib.rs:147-155`
  (`session_id`, `requested_at: DateTime<Utc>`, ...), outside F09's owned
  paths (`crates/jcode-app-core/src/tool/selfdev/**`,
  `crates/jcode-build-support/src/**`).
- Proposal: add `crates/jcode-selfdev-types/src/**` to F09's owned paths.
  Note `requested_at` already provides the timestamp; the missing piece is
  initiating-session liveness (completion/rollback at
  `crates/jcode-build-support/src/lib.rs:236/:254` match `session_id` but never
  check process liveness).

## GN-3: new node - reuse, do not duplicate, the existing MCP child cap (scope guard for F12)

- `OwnedChildPermit` with `MAX_OWNED_MCP_CHILDREN` already exists at
  `crates/jcode-base/src/mcp/client.rs:39-63` and is attached to clients at
  `client.rs:174/:322`. F12's text ("Add configurable global caps") reads as
  greenfield; implemented blind it would produce two competing counters.
- Proposal: either fold into F12's acceptance gates ("extends OwnedChildPermit;
  no second cap mechanism introduced") or add a small explore child of F12
  that maps every acquire/release site before implementation. Background tasks
  genuinely have no cap (no `MAX|cap|limit` in
  `crates/jcode-base/src/background.rs`), so that half is greenfield.

## GN-4: new node or F26 scope note - startup PID sweep already exists

- `sweep_stale_pid_markers` exists at
  `crates/jcode-storage/src/active_pids.rs:362` and is already called at
  startup from `crates/jcode-base/src/session.rs:66`, guarded by
  `!persistence_failed` with an explicit ordering comment (session.rs:62-65).
- F26's first gate ("Dead markers without session JSON disappear at startup")
  may already pass. Proposal: F26 should begin with a verify step that fixtures
  the existing sweep before writing new code, and its remaining implementation
  scope is (a) the periodic re-sweep, (b) telemetry marker liveness
  (`prune_active_session_files` at
  `crates/jcode-telemetry-core/src/state_support.rs:165` prunes by 24h file age
  only, no PID check), and (c) duplicate removal. The duplicate is confirmed
  real and non-trivial: `crates/jcode-app-core/src/telemetry_state.rs` is not
  declared in `crates/jcode-app-core/src/lib.rs` (uncompiled) and textually
  DIFFERS from the compiled `state_support.rs` copy (repo-detection helpers at
  state_support.rs:266-309 vs `crate::build::get_repo_dir()` at
  telemetry_state.rs:279/:286), so the equivalence review F26 mandates is
  necessary, not a formality.

## GN-5: new node - background status writes also swallow serialization errors (confirmed F04 scope, flag for gate wording)

- `write_status_file` at `crates/jcode-base/src/background.rs:133` both writes
  non-atomically AND silently drops `serde_json::to_string_pretty` failure and
  `fs::write` failure (`if let Ok(json) ... let _ = fs::write`). F04's first
  gate covers atomicity; "surface persistence failures" is in its content but
  not in a gate. Proposal: the coordinator adds an explicit F04 gate
  "status-serialization and write failures are surfaced, not swallowed" so the
  verify node F05 tests it.

## GN-6: observation, no node needed - F14's scripts glob is a forward reservation

- `scripts/**/*lifecycle*` matches nothing today (only
  `tests/e2e/windows_lifecycle.rs` matches anywhere). This is where F14 will
  create its harness, so it is a reservation, not drift. Recorded so a later
  auditor does not flag it as stale.

## Explicitly found NOT to be gaps

- Homebrew weak host identity is still present exactly as F22 claims
  (`.github/workflows/release.yml:436` `StrictHostKeyChecking=no`), while the
  AUR path is already strict (release.yml:541-545): F22's scope is correctly
  narrowed to the Homebrew path.
- Swarm terminal MEMBER retention already exists
  (`crates/jcode-app-core/src/server/swarm.rs:327`,
  `swarm_persistence.rs:376/:434`); F25's retention gate is about the control
  LOG (`swarm_persistence.rs:197/:204/:548`, never truncated), which is still
  real. F25 implementers must not re-add member retention.
- Budget scripts pre-exist (`scripts/check_*_budget.*`, 7 checkers plus data
  files); F23 is correctly an extension (trend/zero-growth), not creation.
