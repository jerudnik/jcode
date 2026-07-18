# Ideal-base acceptance standard

The ideal TUI/CLI foundation is accepted only at one fixed committed source head
and one recorded runtime/evidence state. Partial completion must use a narrower
label.

## A0. Runtime ownership

- Every normal daemon exit path invokes one bounded shutdown authority.
- Pooled MCP children receive owner identity and cannot survive parent death past
  the documented grace period.
- SIGTERM, persistent idle exit, temporary-owner exit, reload, and parent SIGKILL
  fixtures leave zero owned descendants.
- Socket, debug socket, lock, hash, metadata, and registry sidecars are either
  live and coherent or fully removed.

## A1. Work-aware lifetime

- Idle exit requires zero clients and zero active work leases.
- Provider turns, headless/restored turns, debug jobs, background jobs, MCP calls,
  and swarm waiters are covered by the lease authority.
- A no-provider blocking fixture proves the daemon remains alive while each lease
  class is held and exits only after release plus timeout.

## A2. Durable background and recovery state

- Background task status writes are atomic, serialized per task, and error-aware.
- Terminal state cannot be lost to concurrent progress/delivery updates or a torn
  write.
- Malformed or orphaned task state is visible and reconciled without fabricating
  success.
- Stale selfdev pending activation completes or rolls back from candidate identity
  and session liveness while preserving a live canary.
- Disconnect lock timeout leaves a durable cleanup intent that startup can
  reconcile to terminal session truth.

## A3. MCP health and resource bounds

- Killed and hung pooled MCP clients are detected within a bounded interval.
- Stale handles and advertised tool caches are evicted before one cooldown-limited
  reconnect attempt.
- Shared MCP children and live background tasks have configurable global caps,
  metrics, clear refusal errors, and leak-free release on completion.

## A4. Authoritative deterministic validation

- Intended Linux tests block push/PR.
- Intended macOS library tests block push/PR.
- Deterministic `jcode-tui` tests execute rather than remaining compile-only.
- Every ignored test has an explicit live, platform, GUI, helper, or performance
  reason. Unclassified ignores are rejected.
- A real-process matrix covers repeated cancellation, reload handoff, client
  re-exec/resume, invalid replacement, stale markers, retry classification, and
  restart reconciliation with zero residue.

## A5. Package and updater integrity

- Relevant pull requests perform a real Nix package build and launch the result.
- The installed package serves `web/jcode-mobile` from an executable-adjacent or
  declared share path while running outside a source checkout.
- Hermetic installer and Rust updater fixtures cover checksum absence/mismatch,
  interrupted acquisition policy, invalid replacement, rollback, and exactly-once
  daemon reload.
- Every failed acquisition or activation preserves the prior working runtime and
  channel pointers.

## A6. Security, quality, and provenance

- Critical lifecycle, persistence, updater, provider-infrastructure, and TUI paths
  have zero-growth panic/swallowed-error/oversize budgets plus explicit downward
  targets.
- Every accepted security advisory has an owner, rationale, retirement condition,
  and expiry enforced by CI.
- Homebrew publication uses pinned GitHub host identity rather than disabling host
  checking.
- Reproducible artifact scope is explicit, compatibility inputs are pinned, and
  artifacts carry verifiable source/version provenance and an SBOM.

## A7. Durable-state hygiene

- Malformed swarm snapshots and control logs emit structured diagnostics and are
  quarantined without panicking.
- Terminal swarm control logs have a bounded retention or archive policy.
- Dead active-PID markers are swept at startup and periodically.
- Telemetry active-session markers use PID/process liveness rather than a 24-hour
  mtime approximation.
- The tracked but uncompiled app-core telemetry duplicate is removed after an
  equivalence check.

## A8. Gated validation honesty

Each external gate is recorded as `accepted` or `authorization_blocked` with a
named reason and required next action:

- `aarch64-linux` build/smoke or an explicit best-effort support downgrade.
- Minimal scheduled provider-doctor coverage and a fresh full pre-release tier.
- Mobile/iOS simulator or device attach validation.
- Windows and FreeBSD scheduled compile/install smoke if support remains advertised.
- Disposable draft-release acquisition and live updater smoke.

No blocked gate may be described as passing.

## A9. Final signoff

- All mandatory deterministic work-graph nodes are `accepted`.
- Every accepted node cites a commit, evidence path, validation output, edge cases,
  open questions, confidence, and what was not checked.
- The full deterministic matrix passes twice from a clean state at the same commit.
- No owned process, socket, marker, temporary directory, pending activation, or
  cleanup record leaks from the final matrix.
- An independent Opus-class reviewer reports no unresolved blocker omission,
  material mis-ranking, or false validation claim.
- The worktree is clean after the bounded signoff commit. Nothing is pushed unless
  separately authorized.

## Permitted labels

| State                                                         | Strongest honest label                                              |
| ------------------------------------------------------------- | ------------------------------------------------------------------- |
| Current baseline                                              | Core-runtime validated                                              |
| Mandatory nodes accepted, external gates blocked and recorded | Ideal TUI/CLI foundation; external product gates explicitly blocked |
| Mandatory and all advertised external gates accepted          | Unqualified ideal-base signoff for the advertised surfaces          |
