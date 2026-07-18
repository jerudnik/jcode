# F03 independent verification review

## Verdict

**PASS.**

Reviewed exact commit `d8c223d29e35ee8b3b37070686fb1be19cf8b799` (`F03: lease-class fixture matrix, state-machine race tests, watch-send fix`) at HEAD `d8c223d29e35ee8b3b37070686fb1be19cf8b799`.

Reviewer route: **OpenAI `gpt-5.6-sol`, high effort**.

Both F03 acceptance gates are honestly met for the intended no-provider lease matrix. All eight authoritative `ActivityClass` variants were held past the short temporary-idle timeout, released, followed by coordinator exit code 44 and zero socket/hash/metadata residue. The harness reaped every daemon it started, and an independent post-run process census found no surviving fixture daemon. The exact-commit forced-exit path produced code 70 and a durable `fired` marker; a SIGKILL successor booted over stale socket state and then exited cleanly with zero residue.

The F03 changes also fix a real production lost-publication defect: `watch::Sender::send` can fail when there are no receivers and does not retain the supplied value, while `send_replace` stores it regardless. `wait_terminal` subscribes and checks the currently stored value before awaiting a change, so terminal publication is now safe whether the subscriber arrives before or after cleanup completes.

I found no blocking defect in the F03 test surfaces or in the production changes they introduced. Two evidence-strength limitations remain: the process harness does not directly time/assert survival for a full new idle window after release despite labeling that result, and its forced-exit case does not census/reconcile residue in the same runtime directory. Neither defeats the literal F03 gates because the per-class hold/release and clean-exit residue matrix passes, the pure idle-clock fixture proves epoch reset, and abrupt-residue successor recovery is separately exercised. They should nevertheless be tightened in follow-up.

## Validation performed

### Exact source and evidence review

- Confirmed `HEAD == d8c223d29e35ee8b3b37070686fb1be19cf8b799` and the worktree was clean before creating this review.
- Read `git show d8c223d29` completely, including all nine changed paths.
- Read `docs/fork/ideal-base/evidence/F03/README.md`, `lease_class_fixtures_run.log`, `lease_class_fixtures.sh`, `shutdown_fixture_tests.rs`, and all changed `shutdown.rs` regions in context.
- Verified the F03 SHA-256 ledger against all three evidence files. All hashes match.
- Confirmed the harness class list exactly matches `ActivityClass::ALL`: ClientConnection, ProviderTurn, StartupRecovery, DebugJob, BackgroundTask, McpCall, SwarmWaiter, and ScheduledDelivery.

### In-process shutdown suite

Ran:

`scripts/dev_cargo.sh test -p jcode-app-core --lib server::shutdown`

Result:

- **23 passed; 0 failed; 1126 filtered out; 2.01s**.
- The 23 consist of the prior 11 pure tests plus all 12 new coordinator fixtures.
- The passing fixtures cover:
  - `begin_and_wait` terminal publication;
  - idle refusal while leased and acquisition remaining open after refusal;
  - 50 iterations of concurrent idle claims versus lease acquisition;
  - drain-until-release and deadline abandonment;
  - weaker-reason supersession, stronger-reason upgrade, and non-extending deadlines;
  - all 16 ordered pairs over SigTerm, ReloadExecFailed, AcceptLoopFailure, and TemporaryOwnerExit;
  - temporary reload refusal and reload refusal during termination;
  - reload-to-SIGTERM mid-drain upgrade;
  - reload handoff after release;
  - reload exec-failure re-entry with exit code 42;
  - 16 concurrent terminal waiters observing one outcome.

### Exact-commit binary rebuild and runtime matrix

The pre-existing `target/selfdev/jcode` was not from F03, so I rebuilt it:

`scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode`

The build itself succeeded. A trailing BSD/Linux `stat` syntax mismatch made the compound background command report failure, so I independently verified the artifact afterward:

- `target/selfdev/jcode --version` -> `jcode v0.46.0-dev (d8c223d29)`;
- embedded build version -> `v0.46.0-dev (d8c223d29)` with no dirty marker;
- binary mtime -> `2026-07-18T12:40:29-04:00`, after the reviewed commit;
- binary SHA-256 -> `933f46aea02b32b0bd1bd895d3875df4c3323d6fe47381f07edad51876baef7e`.

Ran against that exact-commit binary:

`bash docs/fork/ideal-base/evidence/F03/lease_class_fixtures.sh target/selfdev/jcode`

Result: **exit 0, all assertions passed, 278.73s**.

Observed output:

- all 8 lease classes: daemon alive after 18 seconds with a 5-second idle timeout;
- all 8 lease classes: exit code 44 after release;
- all 8 lease classes: zero socket/hash/metadata residue;
- forced cleanup hang + SIGTERM: watchdog exit code 70;
- forced path: durable marker contains `"event":"fired"`;
- parent SIGKILL: stale socket observed;
- successor: booted over stale residue, idle-exited 44, zero residue;
- final line: `ALL F03 FIXTURES PASSED`.

A post-run `ps` census found no remaining `target/selfdev/jcode serve` or fixture harness process.

### Adversarial semantic checks

#### `send_replace` lost-wakeup fix

The prior code called `let _ = self.terminal_tx.send(Some(outcome))`. Tokio watch channels are initialized with a sender and an initial value but no receiver here. If cleanup completed before `wait_terminal` subscribed, `send` returned `Err` because no receivers existed and the new outcome was not retained. `begin_and_wait` then subscribed to the still-`None` channel and could await forever.

The replacement at `shutdown.rs:941` uses `send_replace(Some(outcome))`, which always replaces the stored value. `wait_terminal` at `:850-859` subscribes, immediately calls `borrow_and_update`, returns a pre-stored outcome if present, and only otherwise awaits `changed`. This closes both publication orders without a check/subscribe race.

#### Instance-scoped authority and watchdog behavior

- `ShutdownCoordinator::new(authority)` stores the supplied authority and sets `watchdog_enabled: true`.
- The process-global `coordinator()` constructs the coordinator with `Arc::clone(typed_authority())`, the same authority returned by global acquisition and lifecycle snapshot helpers. Production lease behavior is therefore unchanged.
- Only `leaked_for_test`, behind `#[cfg(test)]`, creates a private authority and mutates `watchdog_enabled` to false.
- All three watchdog arm sites, first begin, stronger upgrade, and reload exec failure, are gated by the instance field. Every production instance has the default true value; no production configuration or environment toggle can disable it.
- The real watchdog remains validated at process level by the exact-commit exit-70 fixture.

#### 50-iteration idle race

Each iteration creates a fresh coordinator/authority pair, races four acquirers performing up to five acquisitions each against twenty idle-claim attempts, and deliberately retains every successfully acquired token. If the claim is accepted, the test proves both that post-claim acquisition is refused and that no racer holds a token. Because successful racer tokens are never released before that assertion, `all_tokens.is_empty()` is a meaningful strong invariant rather than a vacuous final-state check. If acquisition wins, the claim remains refused. The fixture therefore exercises both sides of the table mutex linearization.

#### Sixteen pairwise races

The fixture iterates the full 4x4 ordered matrix and starts both `begin` calls concurrently on a multi-thread runtime. It requires at least one accepted begin, awaits one bounded stored terminal outcome, requires its reason to be one of the racers, and, when both calls report accepted, requires the stronger priority to drive. The test does not count cleanup side effects directly, but the coordinator's single terminal publication and first-acceptance executor structure are exercised for every pair.

#### Debug fixture surface

The commands are placed before session snapshot/lookup in `debug_server_state.rs`, making them genuinely session-independent. They still execute only after `Request::DebugCommand` passes `debug_control_allowed()` in `debug.rs`. That existing gate requires configured debug control, `JCODE_DEBUG_CONTROL`, or the established file toggle. The surface does not bypass authentication/session resolution beyond the intentionally privileged debug-command channel, which already exposes much broader mutation capabilities. Unknown classes fail parsing, acquisition preserves typed `ShuttingDown` refusal, and guards are retained in a process-global map until token release or process exit.

## Findings

### Blocking

None.

### Important but nonblocking

#### F03-I1: the harness does not directly assert a full new idle window after release

Lines 130-138 release the token and then wait up to 40 seconds for exit 44. They do not assert that the daemon remains alive for any minimum post-release interval. Therefore the output text `exit 44 after release + full idle window` overstates what that process assertion alone proves: an immediate post-release exit would also pass.

This does not fail gate 1, whose literal requirement is alive while held and exit after release. The common `IdleClock` pure test independently proves that a non-quiescent interval resets the epoch, and the real daemon behavior in the rerun was consistent with the expected polling delay. Strengthen the harness by recording release time, asserting liveness through a lower bound derived from the 5-second timeout, then checking bounded exit.

#### F03-I2: forced-exit residue is discarded rather than censused or recovered in place

The forced-exit fixture checks code 70 and the marker, then immediately deletes its runtime and home directories. It does not report socket residue or launch a successor in that same directory. The SIGKILL successor fixture proves stale-socket recovery in a separate abrupt-exit case, not specifically from watchdog exit.

This does not fail the F03 clean-residue gate as applied to the eight normal lease-release exits and the successor's terminal state, all of which are clean and process-reaped. Forced exit cannot perform normal cleanup by construction. Still, the README statement that forced residue is owned by next-boot reconciliation would be stronger if the forced fixture reused its own directory for a successor and then ran `residue_check`.

### Coverage limitations, not defects

- The eight runtime cases acquire synthetic guards through the debug surface. They prove every class participates correctly in the authoritative lease table and idle gate, but they do not invoke a real provider, restored/headless provider turn, actual debug job, real background tool, real MCP server, live swarm waiter, or actual scheduled-delivery dequeue.
- There is no process-level reload success, temporary reload refusal, mid-drain upgrade, handoff, or reload-exec-failure fixture. Those transitions are exercised in-process.
- There is no process-level accept-versus-idle-claim race, real client admission race, Windows fixture, or repeated stress run of the full runtime matrix.
- Owned descendant-process cleanup belongs to the later integrated F08 gate and is not established by F03's daemon-process census.

## Gate checklist

| Acceptance gate | Result | Evidence |
|---|---|---|
| Short-timeout no-provider fixtures remain alive for each lease class and exit after release | **PASS** | The exact-commit runtime matrix enumerates all eight `ActivityClass::ALL` variants. Each daemon remained alive for 18 seconds against a 5-second timeout while its guard was held, then exited 44 after token release. The debug acquisition path uses the same global authority as production work. |
| No socket/process residue | **PASS** | Every normal per-class exit passed socket/hash/metadata residue checks after the child was reaped. The SIGKILL successor booted over stale state and finished with zero residue. The harness completed with no surviving fixture daemon in an independent process census. Forced watchdog termination intentionally cannot clean before exit; its same-directory recovery remains the nonblocking F03-I2 evidence gap. |

## What I did not check

- I did not run the full `jcode-app-core --lib` suite, workspace suite, Miri, Loom, sanitizers, or model checking.
- I did not run real external providers or make billable/network provider calls.
- I did not start a real MCP child/server, perform a real scheduled delivery, restore a real session into a provider turn, or exercise a real background-tool process.
- I did not run process-level reload fixtures or Windows-specific socket/signal behavior.
- I did not repeat the 4.6-minute runtime matrix multiple times or add timing instrumentation to its release phase.
- I did not inspect every unrelated production file changed between F02 and F03's prerequisite bookkeeping commits; I reviewed the exact F03 commit and its changed runtime/test surfaces.

## Confidence

**High (97%).** The exact-commit unit and runtime suites both pass independently, the watch-channel defect and fix are directly established from Tokio semantics and the local publication order, the authority/watchdog refactor preserves production construction, and every authoritative lease class is exercised through the real daemon authority. Confidence is below 100% because the runtime matrix uses synthetic guards rather than real work initiators, does not directly time the post-release epoch, and does not reconcile forced-exit residue in the same directory.
