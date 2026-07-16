# R04 pilot-prerequisite implementation review

**Reviewer:** independent Fable-style adversarial review  
**Worktree:** `/Users/jrudnik/labs/jcode-fix-r04-marker`  
**HEAD reviewed:** `0f8bd8d9f5556accfebf522577d40930ac9eac47` (`docs(r04): record marker observation remediation`)  
**Base:** `1b9d6e09fe324123ba97b2f627934169484e835b` (`docs(recovery): complete Phase 2 ledger gate`)  
**Range:** `1b9d6e09f..0f8bd8d9`  
**Mode:** read-mostly review plus narrow unit-test filters. No source edits. No live daemon, network, credentials, or destructive actions.

## Verdict: PASS for the narrow R04 pilot prerequisite, with IMPORTANT follow-ups

I found no CRITICAL blocker in the requested R04 source-fix scope. The implementation moves terminal persistence before marker cleanup, makes marker removal conditional on unchanged content plus identity, preserves replaced live successor markers, keeps stale/dead-owner retry evidence when persistence fails, covers disconnect and dead-owner paths with deterministic fixtures, and appends the R04 ledger without rewriting prior blocked history.

This PASS is narrow. It does not approve a general lifecycle/presence redesign, and it does not mean every caller propagates terminal persistence failures to its own caller. Several callers intentionally log and continue runtime cleanup.

## CRITICAL findings

None.

## IMPORTANT findings

### IMPORTANT-1: Disconnect cleanup treats terminal persistence failure as logged-but-successful cleanup

**Evidence:**

- `cleanup_client_connection` removes the agent from `sessions` before acquiring the agent lock: `crates/jcode-app-core/src/server/client_disconnect_cleanup.rs:124-129`.
- On `mark_closed` / `mark_crashed` failure, it logs only and continues: `:132-160`.
- It then emits runtime memory events, removes swarm/member state, clears file touch state, removes shutdown/background/interrupt queues, aborts the processing task, aborts the event task, and returns `Ok(())`: `:163-284`.
- The tests intentionally assert this behavior: on forced save failure, persisted session remains `Active`, successor marker remains, but swarm/session/shutdown runtime state is gone: `client_disconnect_cleanup.rs:513-545` and `:547-580`.

**Why this matters:** The save failure is observable in logs, and marker retry evidence is retained, which satisfies the core marker-safety requirement. But callers of `cleanup_client_connection` cannot distinguish full terminal persistence from partial cleanup because the function returns `Ok(())` in both cases. Any pilot gate that requires caller-propagated terminal persistence must not treat disconnect cleanup success as proof that the terminal session state was saved.

**Required correction if stronger propagation is required:** Return a structured cleanup outcome, or return an error after completing non-terminal cleanup when terminal persistence fails. At minimum, document at the call boundary that `Ok(())` means runtime cleanup completed, not that terminal persistence succeeded.

### IMPORTANT-2: Streaming-marker partial replacement is implemented but not directly covered by a named fixture

**Evidence:**

- `SessionPidMarkerObservations` captures both `active` and `streaming`: `crates/jcode-storage/src/active_pids.rs:115-119`.
- `remove_session_pid_markers_if_unchanged` independently conditionally removes active and streaming markers: `:204-227`.
- Tests cover active-marker replacement and observation instability: `active_pids.rs:656-710`; dead-owner successor tests use `active_marker_path`: `session_tests/cases.rs:133-203`; disconnect successor tests use active-marker assertions: `client_disconnect_cleanup.rs:499-510`, `:513-604`.
- I did not find a fixture that replaces only the streaming marker, or that proves a partial outcome where active is preserved while stale streaming is removed, or active is removed while replaced streaming is preserved.

**Why this matters:** The source shape is symmetric and likely correct, but multi-marker partial outcomes were explicitly in scope. A future regression could accidentally couple active/streaming removal or ignore streaming identity without the current tests catching it.

**Required correction:** Add direct tests for both marker kinds: replaced active + unchanged streaming, unchanged active + replaced streaming, and both replaced. Assert exact `SessionPidMarkerRemoval` booleans and file survival/removal.

### IMPORTANT-3: PID-marker file locking is fail-closed on acquisition failure, but not timeout-bounded

**Evidence:**

- `PidMarkerLock::acquire` uses blocking `fs2::FileExt::lock_exclusive(&file)` with no timeout: `crates/jcode-storage/src/active_pids.rs:23-35`.
- Terminal flows call marker observation before saving terminal state: `Session::mark_closed_and_persist` observes at `crates/jcode-base/src/session.rs:1090-1093`; `mark_crashed_and_persist` observes at `:1096-1102`.
- A lock-open/acquisition failure returns default/no-op and tests verify marker state stays untouched: `active_pids.rs:187-188`, `:211-212`, and `:740-760`.
- Agent-lock timeout is covered separately and leaves session/marker state Active while runtime cleanup continues: `client_disconnect_cleanup.rs:607-631`.

**Why this matters:** The requested “lock timeout fail-closed” behavior is true for the async agent lock, and marker lock acquisition failure is fail-closed. But an indefinitely held marker file lock would block terminal persistence before the save rather than timing out and returning a controlled failure. Advisory locks normally release on process death, so this is a liveness edge, not a demonstrated correctness break.

**Required correction:** If the prerequisite requires bounded terminal-close latency under a wedged live lock holder, replace blocking marker-lock acquisition in terminal paths with a bounded try-lock loop that returns a save-visible error or a no-observation path after timeout.

## Dimension-by-dimension audit

### Terminal persistence ordering

PASS. `Session::persist_terminal_state_with_observed_markers` does replacement test seam, forced-save failure seam, then `self.save()?`, then `remove_session_pid_markers_if_unchanged`: `crates/jcode-base/src/session.rs:1080-1088`. `mark_closed_and_persist` and `mark_crashed_and_persist` observe markers before changing status and saving: `:1090-1102`. `Agent::mark_closed` and `mark_crashed` now return `Result<()>` and call those save-aware methods: `crates/jcode-app-core/src/agent.rs:970-981`, `:1000-1008`.

### Observable save failures

PASS with IMPORTANT-1 boundary. Core APIs propagate errors: `Session::mark_closed_and_persist` / `mark_crashed_and_persist` return `Result<SessionPidMarkerRemoval>` and `Agent::mark_closed` / `mark_crashed` return `Result<()>`. Ambient callers propagate with `?`: `ambient/runner.rs:399`, `:473`, `:922`, `:949`, `:973`. TUI, CLI panic/signal, server startup, and disconnect paths log failures: `commands_review.rs:270-276`, `conversation_state.rs:633-638`, `src/cli/terminal.rs:157-165`, `:176-184`, `server.rs:873-879`, `:933-940`, `client_disconnect_cleanup.rs:132-160`.

### Unchanged-marker conditional removal

PASS. Observations include bytes plus identity: `active_pids.rs:109-113`. Removal re-reads contents and metadata under the marker lock and removes only if both match: `:414-430`. Stale crash-scan cleanup also re-reads bytes under lock and refuses live or changed markers: `:400-412`.

### Replaced-successor preservation

PASS. Same-PID or different-PID atomic replacements are preserved because identity includes inode/dev on Unix plus len/mtime: `active_pids.rs:83-107`, `:414-430`. Tests cover replaced active markers in storage, dead-owner success, dead-owner failure/retry, and disconnect cleanup: `active_pids.rs:656-679`; `session_tests/cases.rs:133-203`; `client_disconnect_cleanup.rs:513-604`.

### Observation identity/content stability

PASS. Observation now occurs while holding the shared marker lock: `active_pids.rs:183-198`. It reads metadata before content, test hook replacement, metadata after, and rejects changed identity: `:140-159`. Remediation addendum records this exact issue and fix: `ledger.md:304-321`. Test `observation_rejects_marker_replaced_between_content_and_metadata_read` passed and covers unstable observation rejection: `active_pids.rs:684-710`.

### Lock timeout fail-closed behavior

PASS for the agent lock path, IMPORTANT for marker-lock liveness. Disconnect cleanup uses a bounded `agent_lock_timeout`: `client_disconnect_cleanup.rs:20-35`, `:128-129`. Timeout logs and skips terminal persistence: `:203-208`. Test verifies persisted state remains Active and marker remains: `:607-631`.

### Disconnect and dead-owner paths

PASS. Disconnect paths classify closed/crashed/reloading at `client_disconnect_cleanup.rs:37-54`, then terminally persist before marker cleanup through Agent methods. Dead-owner reconciliation observes markers, skips if the observed active marker is live, detects crash, persists terminal state with observed markers, and propagates errors: `session.rs:1156-1166`. The top-level reconciler defers global stale sweep if any persistence failure occurred: `session.rs:43-68`.

### Multi-marker partial outcomes

Source PASS, test coverage IMPORTANT gap. `remove_session_pid_markers_if_unchanged` handles active and streaming independently: `active_pids.rs:214-227`. This permits partial outcomes. I did not find direct streaming-marker replacement tests.

### Caller propagation

Mixed. APIs propagate. Ambient propagates. Server/TUI/CLI disconnect-like flows often log and continue. Treat this as acceptable for marker safety only if log-observable failure plus retained marker retry evidence is sufficient.

### Fixture false-pass risks

Mostly controlled. The strongest fixtures assert persisted session status plus marker existence/content, not just return values. Examples: save-failure retains marker and Active status before retry: `session_tests/cases.rs:93-131`; replaced successor success/failure: `:133-203`; disconnect save failures: `client_disconnect_cleanup.rs:513-580`; lock timeout: `:607-631`. Remaining false-pass risk is that disconnect tests do not assert logs or structured error propagation, because the implementation currently has neither.

### R11 append-only ledger truth

PASS. R11 requires append-only amendments and no replacement of prior decisions: `docs/fork/recovery/seams/R11-documentation-governance/ledger.md:18`, `:26`; `docs/fork/recovery/PROGRESS.md:3`. The R04 ledger diff is append-only: `git diff --numstat 1b9d6e09f..HEAD -- docs/fork/recovery/seams/R04-session-process-background-lifecycle/ledger.md` returned `70 0`. The new R04 section is appended after the prior blocked state and explicitly says a fresh independent source-fix sign-off is still required before changing authoritative status: `ledger.md:246-253`, `:298-302`, with remediation addendum at `:304-321`.

## Commands and results

Baseline and diff:

- `pwd && git rev-parse HEAD && git rev-parse 1b9d6e09f && git status --short && git diff --name-status 1b9d6e09f..HEAD`
  - Result: worktree `/Users/jrudnik/labs/jcode-fix-r04-marker`, HEAD `0f8bd8d9f5556accfebf522577d40930ac9eac47`, base `1b9d6e09fe324123ba97b2f627934169484e835b`, clean status, 15 changed files.
- `git diff --stat 1b9d6e09f..HEAD && git diff --check 1b9d6e09f..HEAD`
  - Result: 15 files, 943 insertions, 56 deletions. `diff --check` exit 0.
- `git diff --numstat ... R04 ledger`
  - Result: `70 0`, append-only.

Tests run successfully:

- `scripts/dev_cargo.sh test -p jcode-storage observation_rejects_marker -- --nocapture`
  - Result: 1 passed.
- `scripts/dev_cargo.sh test -p jcode-base stale_reconcile -- --nocapture`
  - Result: 2 passed.
- `scripts/dev_cargo.sh test -p jcode-app-core lock_timeout_is_observable -- --nocapture`
  - Result: 1 passed, with one unrelated dead-code warning.
- `JCODE_DEV_CARGO_ALLOW_ZERO_TESTS=1 scripts/dev_cargo.sh test -p jcode-base reconcile_save_failure -- --nocapture`
  - Result: 1 passed.
- `scripts/dev_cargo.sh test -p jcode-app-core disconnect_save_failure_retains_successor -- --nocapture`
  - Result: 2 passed, with one unrelated dead-code warning.
- `scripts/dev_cargo.sh test -p jcode-app-core idle_closed_disconnect -- --nocapture`
  - Result: 1 passed, with one unrelated dead-code warning.
- `scripts/dev_cargo.sh test -p jcode-storage lock_failure_leaves_marker_state_untouched -- --nocapture`
  - Result: 1 passed.
- `scripts/dev_cargo.sh test -p jcode-storage explicit_sweep_removes_dead_marker_without_session_data -- --nocapture`
  - Result: 1 passed.
- `JCODE_DEV_CARGO_ALLOW_ZERO_TESTS=1 scripts/dev_cargo.sh test -p jcode-base crash_scan_cleans_invalid_and_orphaned_dead_markers -- --nocapture`
  - Result: 1 passed.
- `scripts/dev_cargo.sh test -p jcode-storage conditional_cleanup_preserves_a_replaced_live_marker -- --nocapture`
  - Result: 1 passed.
- `scripts/dev_cargo.sh test -p jcode-storage stale_marker_sweep_removes_dead_and_invalid_but_preserves_live -- --nocapture`
  - Result: 1 passed.
- `JCODE_DEV_CARGO_ALLOW_ZERO_TESTS=1 scripts/dev_cargo.sh test -p jcode-base reconcile_active_sessions_marks_dead_pid_crashed -- --nocapture`
  - Result: 1 passed.
- `JCODE_DEV_CARGO_ALLOW_ZERO_TESTS=1 scripts/dev_cargo.sh test -p jcode-base reconcile_active_sessions_sweeps_dead_marker_without_session_data -- --nocapture`
  - Result: 1 passed.

Invalid/diagnostic commands:

- `scripts/dev_cargo.sh test -p jcode-base reconcile_save_failure -- --nocapture` first returned exit 97 after the intended unit test passed because the wrapper rejects zero-test integration binaries unless `JCODE_DEV_CARGO_ALLOW_ZERO_TESTS=1` is set. Rerun with the env var passed.
- Two attempted cargo commands with multiple test-name filters returned usage errors. They were rerun as split one-filter commands and passed.

Final cleanliness:

- `git status --short`
  - Result: clean.

## Confidence and gaps

**Confidence:** high for the narrow R04 prerequisite. Source, ledger, and focused tests align.

Gaps:

- I did not run full `check -p jcode --bin jcode` or full test suites.
- I did not exercise a live daemon, real disconnecting TUI, credentials, network, or OS-level hung advisory lock.
- I did not verify runtime log emission contents, only source paths and tests.
- Streaming-marker replacement partial cases are source-reviewed but not fixture-proven.

## Required follow-up

Before widening beyond this prerequisite, add direct streaming-marker partial-outcome tests and clarify disconnect cleanup’s return contract. If caller-observable terminal persistence is a hard requirement, change `cleanup_client_connection` to return a structured partial-failure outcome instead of only logging.
