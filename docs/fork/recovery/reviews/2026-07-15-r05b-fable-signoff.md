# Fable sign-off: R05B worker dispatch/reclaim ledger

- **Verdict:** **PASS for the authoritative R05B ledger as a blocked/retain-fork ledger.**
- **Seam approval / swarm widening:** **FAIL / blocked**, as the ledger itself states. This is not a contradiction: the ledger is acceptable because it preserves the evidence and correctly blocks approval until the listed fixes and fixtures land.
- **Exact commit reviewed:** `aa8a7b39e` (user-supplied authoritative commit for `/Users/jrudnik/labs/jcode-seam-r05b`).
- **Scope:** Read-only repository/source review. I did **not** read any Sol sign-off. I did **not** run shell commands, cargo, live terminal/process, network, or credentialed actions.

## IMPORTANT / CRITICAL findings

### CRITICAL

None against the ledger.

### IMPORTANT

1. **Ledger correctly preserves the required independent review/adjudication shape, but preservation hashes were not recomputed in this sign-off.**  
   Evidence: the ledger records `opus-review.md` and `grok-review.md` as byte-preserved copies with SHA-256 hashes and Terra `shasum`/`cmp` reproduction (`docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md:15-20`). Both preserved review files exist and record independent review heads and constraints (`opus-review.md:1-14`, `grok-review.md:1-18`). Under the no-live-process constraint, I did not recompute SHA-256 or `cmp`; I treat Terra's recorded reproduction as preserved evidence, not a fresh Fable command result.

2. **Retain-fork plus blocked approval is the right disposition.**  
   Evidence: the ledger states `State = blocked`, `Authority today = fork`, and `Recommended disposition = retain-fork` (`ledger.md:3-13`). Its fork-provenance checkpoint says `RunPlanChurnGuard` and `reclaim_stale_plan_assignments` are fork-only while the `Inline` default already existed at base/upstream (`ledger.md:37-38`, `ledger.md:46-55`). Current fork source confirms the load-bearing fork mechanisms exist: `RunPlanChurnGuard` aborts at three waves (`crates/jcode-app-core/src/tool/communicate.rs:559-622`) and `reclaim_stale_plan_assignments` is wired before assignment (`crates/jcode-app-core/src/server/comm_control.rs:641-697`, `comm_control.rs:1647-1652`).

3. **The dispatch/reclaim/liveness/timer/spawn-mode/history census is materially complete and line-cited.**  
   Evidence: direct assignment writes `assigned_to`, `queued`, and replaces progress (`comm_control.rs:1723-1739`); `assign_next` reclaims stranded work and spawns with `spawn_mode = None`, inheriting config (`comm_control.rs:1993-2047`); auto-pick claims have a 15s TTL (`comm_control.rs:86-127`); canonical dead status is `failed|stopped|crashed` (`swarm.rs:369-374`); dead-PID sweep mirrors crashed sessions (`swarm.rs:265-322`); stale/reap/salvage timers are configured and used (`swarm.rs:146-154`, `swarm.rs:221-247`, `swarm.rs:647-806`); event history is capped at 5000 (`state.rs:380-381`, `swarm.rs:1418-1441`); member capacity is live-member bounded at `MAX_SWARM_MEMBERS` (`comm_session.rs:1434-1502`).

4. **Visible versus Auto semantics are a real blocker, and the ledger correctly fails swarm widening on it.**  
   Evidence: `SwarmSpawnMode` distinguishes `Visible` from `Auto` in the public enum (`crates/jcode-config-types/src/lib.rs:632-664`), but `spawn_swarm_agent` groups `Visible | Auto` into the same visible attempt (`comm_session.rs:619-648`) and sends both `Ok(false)` and `Err(_)` into headless fallback (`comm_session.rs:651-691`). The ledger identifies this as a BLOCKER and requires explicit `Visible` to fail closed while only `Auto` may visibly fallback (`ledger.md:80`, `ledger.md:90-93`, `ledger.md:107-115`).

5. **Direct stale assignment and cap-fail checkpoint history defects are real and correctly block approval until fixed/tested.**  
   Evidence: direct assignment allows stale takeover by only rejecting active/fresh conflicts (`comm_control.rs:1660-1687`) and then inserts a defaulted `SwarmTaskProgress`, losing prior fields (`comm_control.rs:1727-1738`). The primitive reclaim preserves some progress fields but overwrites `checkpoint_summary` (`crates/jcode-plan/src/lib.rs:618-640`), and its test proves heartbeat retention but does not prove prior checkpoint preservation (`jcode-plan/src/lib.rs:1104-1151`). Cap-fail salvage overwrites `checkpoint_summary` (`swarm.rs:445-462`), and the cap-fail fixture asserts only outcome/status/assignment (`swarm.rs:3107-3138`). The ledger's history-preservation audit and required fixtures cover these exact gaps (`ledger.md:43`, `ledger.md:81-82`, `ledger.md:92-95`).

6. **R04/R05A boundaries are preserved as policy boundaries, with one structural liveness-copy defect.**  
   Evidence: `RESPONSIBILITIES.md` says R04 owns process/task life, R05B owns assignment/reclaim/retry policy, and reclaim cannot erase history (`docs/fork/recovery/RESPONSIBILITIES.md:61-70`). The R05B ledger records the exact R04/R05A/R05B boundary (`ledger.md:21-34`). Source matches the layering: R05B consumes control-log fold membership in `reclaim_stale_plan_assignments` (`comm_control.rs:630-660`) and consumes R04-fed member death via `member_status_is_dead` and dead-PID sweep (`swarm.rs:265-322`, `swarm.rs:369-374`). The structural defect is the copied dead-status triple in lazy reclaim (`comm_control.rs:732-738`) and the documented third copy in verbs, which the ledger flags as IMPORTANT (`ledger.md:41`, `ledger.md:67`, `ledger.md:79`, `ledger.md:95`).

7. **Strict no-swarm pilot exclusion and swarm-widening stops are correctly stated.**  
   Evidence: the bounded pilot prerequisite explicitly excludes R05B unless the chosen stack exercises swarm behavior (`RESPONSIBILITIES.md:72-88`). The R05B ledger repeats that it is not a prerequisite for the smallest no-swarm pilot and forbids `run_plan`, automatic worker spawning, explicit visible spawning, or dead-worker reclaim in a pilot until fixtures 1-6 pass (`ledger.md:99-104`). This is the correct boundary: do not block a no-swarm, no-tool one-turn pilot on R05B, but do block any swarm-driven pilot.

8. **R09/no-update obligations and debt visibility are preserved, with per-file R05B debt enumeration still a required implementation-gate task.**  
   Evidence: R09 forbids blanket `--update` and requires red debt to remain visible and behavior-owned (`docs/fork/recovery/seams/R09-quality-gates/ledger.md:24-30`, `R09-quality-gates/ledger.md:46-57`). R05B ledger repeats no `--update`, assigns likely R05B-owned files, and explicitly says per-file production/test-size violations still require enumeration before implementation gate (`ledger.md:117-123`). This is acceptable for a blocked ledger, not for implementation approval.

9. **Required fixtures, slices, rollback/stop conditions, and negative findings are present and appropriately bounded.**  
   Evidence: required fixtures cover Visible/Auto, stale direct takeover, automatic reclaim/cap-fail history, liveness authority, residual session count, and R04-to-R05B chain (`ledger.md:90-98`). Bounded slices include class, acceptance, and rollback/stop conditions (`ledger.md:107-115`). Negative/failure modes checked include wrong default provenance, duplicate assignment, dead/departed reassignment, reclaim cap, churn waves, stale takeover history reset, cap-fail overwrite, liveness drift, residual session growth, and R04-to-R05B chain gap (`ledger.md:117-126`). Preserved Grok negative findings agree on no unbounded automatic dead-worker retry, no concurrent auto-pick double assignment, no foreign zombie reuse, and no silent dead-member salvage (`grok-review.md:222-230`).

## Minor notes

- `docs/fork/recovery/PROGRESS.md` still says R05B remains pending (`PROGRESS.md:12`, `PROGRESS.md:35`). That is stale relative to the present R05B seam directory and ledger. It does not invalidate this sign-off, but the coordinator should update progress after sign-off integration.
- I did not independently verify fixed-ref marker counts (`base/upstream/fork = 0/0/3`, etc.) because that would require `git show`/`git diff` process execution. I verified current-source presence and preserved-review/ledger evidence only.
- I did not run cargo or fixtures. All validation here is static read-only inspection plus preserved command results in the reviews.

## Commands / operations performed

No shell commands were run. No process, terminal, network, or credential operation was used.

Read-only inspections performed with repository tools:

- Read `docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md`.
- Read preserved non-Sol reviews: `opus-review.md` and `grok-review.md`.
- Read source evidence in:
  - `crates/jcode-app-core/src/server/comm_control.rs`
  - `crates/jcode-app-core/src/server/comm_session.rs`
  - `crates/jcode-app-core/src/server/swarm.rs`
  - `crates/jcode-app-core/src/server/state.rs`
  - `crates/jcode-app-core/src/tool/communicate.rs`
  - `crates/jcode-app-core/src/tool/communicate_tests.rs`
  - `crates/jcode-config-types/src/lib.rs`
  - `crates/jcode-plan/src/lib.rs`
- Read governance/overlay evidence in:
  - `docs/fork/recovery/RESPONSIBILITIES.md`
  - `docs/fork/recovery/PROGRESS.md`
  - `docs/fork/recovery/seams/R09-quality-gates/ledger.md`

## Confidence and gaps

- **Confidence:** medium-high. The ledger's main blockers and boundaries reproduce directly in source, and the preserved independent reviews are present and consistent with the adjudication.
- **Gaps:** no hash recomputation, no fixed-ref diff recomputation, no live/cargo/process tests, no Sol sign-off read, no quality-gate per-file enumeration. These gaps are consistent with the user's no-live-process constraint and with the ledger's own blocked status.
