# Phase 1 independent responsibility map

- Repository: `/Users/jrudnik/labs/jcode`
- Role: independent Phase 1 mapper, read-only research
- Measured refs: fork `HEAD=7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream `802f6909825809e882d9c2d575b7e478dce57d3b`, merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`.
- Curated synchronization: `b3ed82a6bc84656518a165d48bfd8253303286a3`, one parent `8ed75637accdd40ded1f1d3ac8ce1390459b8d1f`.
- Budget: seven decisive evidence checkpoints; no repository files, refs, worktrees, or stashes changed.
- Confidence: medium-high overall. Boundaries are researched proposals, not authority or remediation decisions.

## Executive findings

1. The R00-R11 seed index is useful triage but not a coherent behavioral partition. R01, R03, R05, R06, R07, R08, and R09 each mix independently testable authorities or invariants.
2. The strongest split is the live identity/reload authority from reload/session continuity (R01A/R01B), wire compatibility from client lifecycle (R03A/R03B), DAG/control-plane policy from worker dispatch/liveness (R05A/R05B), durable session evidence from memory/backup (R06A/R06B), and gate semantics from debt/ratchet policy (R09A/R09B).
3. R07 must split tool/MCP lifecycle from discovery/telemetry/network consent. R08 must split command/input, render state, session picker, and desktop adaptation. These are not one reviewable authority.
4. Recommended six full seams, and no more: R00, R01A, R02, R09B, R05B, R03A. All other proposed seams are light or defer with explicit escalation triggers.
5. The bounded pilot should not be a broad replay. The smallest safe question is whether one non-secret provider/model route can preserve configuration provenance, model selection, wire identity, session continuity, and trusted gate observations across a disposable fork/upstream comparison.

## Evidence and reproducible checks

### Repository and gate facts

- `docs/fork/recovery/BASELINES.md:48-72,159-224` records the refreshed refs, 406 shared changed paths, stale `vendor/upstream` at merge base, one-parent curated sync, preserved topology, and current red debt.
- `docs/fork/recovery/PRESCREEN.md:17-33,69-85` explicitly says path matching is non-exclusive and that 127 fork and 64 upstream paths remain unclassified. It says counts do not establish authority.
- `docs/fork/recovery/QUALITY_GATES.md:19-31,61-71` distinguishes trusted green gates from real size/panic/swallowed debt, and separates imported/composed/fork-specific changes inside the curated slice.
- `docs/fork/recovery/RESPONSIBILITIES.md:1-28` makes the seed index provisional and caps full review at six.

Reproduction commands:

```bash
git rev-parse HEAD upstream/master vendor/upstream
git merge-base HEAD upstream/master
git rev-list --left-right --count HEAD...upstream/master
git show -s --format='commit=%H parents=%P subject=%s' b3ed82a6bc84656518a165d48bfd8253303286a3
```

### Four distinct equivalence/relationship claims

- **Path overlap**: a path appears in both `git diff --name-only base..fork` and `base..upstream`. It indicates a collision surface only. It is not behavioral agreement. The pre-screen measured 406 shared paths, including 323 under `crates/`.
- **Commit ancestry**: `git merge-base --is-ancestor <commit> HEAD` answers reachability, not authorship or equivalence. The six maintenance commits listed in `PRESCREEN.md:121-132` are ancestors; this closes stale backlog items but does not prove that an upstream commit is present.
- **Patch equivalence**: stable patch IDs compare normalized individual patches. The exact pre-screen command in `PRESCREEN.md:92-103` and an independent rerun over `HEAD` versus `upstream/master` returned an empty intersection. This is a negative finding, not evidence of absent behavior.
- **Semantic equivalence**: requires symbol/contract/test/observable comparison under stated assumptions. The one-parent curated sync can absorb upstream behavior while destroying per-commit identity. No seam below is marked semantically equivalent solely from path overlap or ancestry. No broad semantic equivalence audit was completed in this bounded map.

### Independent checks performed

```bash
# Stable patch-ID intersection, expected empty
base=$(git merge-base HEAD upstream/master)
comm -12 <(git log --no-merges --pretty=format:'commit %H' -p "$base..HEAD" | git patch-id --stable | cut -d' ' -f1 | sort -u) <(git log --no-merges --pretty=format:'commit %H' -p "$base..upstream/master" | git patch-id --stable | cut -d' ' -f1 | sort -u)

python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'  # 17 tests, OK
python3 -m py_compile scripts/rust_production_filter.py scripts/check_panic_budget.py scripts/check_swallowed_error_budget.py tests/test_rust_production_filter.py
git diff --check
```

The fork/upstream diff stat confirms substantial two-sided semantic search surfaces: protocol/reload 1181 fork insertions versus 369 upstream insertions, config/provider 1976 versus 546, and swarm/DAG/control-log 2036 versus 310. These are divergence signals, not dispositions. The full Rust suite, live daemon reproduction, real-provider behavior, and per-symbol semantic equivalence were not run.

## Proposed responsibility map

Each entry uses: **owns; excludes; protected observable invariants; evidence surfaces; operational risk; dependencies; pilot relevance; review; score/rationale; cheapest decisive checks; confidence; not checked**.

### R00 - Integration provenance and sync governance (renamed, retained)

- **Owns:** ref/branch ancestry, merge-base snapshots, curated-sync provenance, synchronization posture, and evidence standards for deciding whether a behavior is imported, composed, fork-specific, or unresolved.
- **Excludes:** runtime authority, provider behavior, gate implementation, and implementation of any sync/remediation slice.
- **Protected invariants:** no broad replay before gates; no automatic upstream authority; dirty prompt, stashes, branches, and worktrees preserved; every equivalence claim names refs, command, and assumptions.
- **Evidence surfaces:** `BASELINES.md:48-72,196-243`; `PRESCREEN.md:87-108,121-132`; `RESPONSIBILITIES.md:1-28`; curated commit `b3ed82a6b` and `git show -s` ancestry command.
- **Operational risk:** Very high. A wrong provenance claim contaminates every later seam and can cause irreversible broad synchronization.
- **Cross-seam dependencies:** all seams, especially R01A, R02, R03A, and R09B.
- **Pilot relevance:** mandatory prerequisite. It defines the disposable refs and stop conditions.
- **Review:** `full`, rank 1, score 16/16. Divergence 4, operational risk 4, contested authority 4, pilot dependency 4.
- **Cheapest decisive checks:** reproduce baseline/ref topology; rerun stable patch-ID intersection; for each pilot symbol compare fork/upstream history and current behavior without treating `vendor/upstream` as current.
- **Confidence:** high for negative ancestry/patch-ID findings, medium for semantic implications.
- **Not checked:** no complete semantic reconstruction of the 221 commits absorbed by the curated sync; 127/64 unclassified paths remain open.

### R01A - Build identity and daemon reload authority (split from R01)

- **Owns:** which executable/build a long-lived daemon or selfdev client may run, source fingerprint/hash/version reporting, published/current/launcher links, pending activation, and reload-target selection.
- **Excludes:** wire compatibility verdict details (R03A), session continuation semantics (R01B/R04), package release policy (R10), and provider selection.
- **Protected invariants:** a daemon must not claim “already newest” while mapped to a stale executable; reload target identity is observable; forced reload can change mapped build while preserving sessions; build/source metadata is internally consistent.
- **Evidence surfaces:** `crates/jcode-build-meta/src/lib.rs:8-32` (`VERSION`, `GIT_HASH`, `BUILD_SOURCE_DIR`); `crates/jcode-selfdev-types/src/lib.rs:19-23,73-114,144-179` (`ReloadRecoveryDirective`, `SourceState`, `PendingActivation`, `BuildInfo`); `crates/jcode-build-support/src/tests.rs:316-417,575-587`; `tests/e2e/binary_integration.rs:113-139,267-373`; incident note summarized in `PRESCREEN.md:115-119` with hash `80012e2c...`.
- **Operational risk:** Very high. The stale-daemon incident is a concrete authority failure across Nix binary, selfdev current, stable channel, and live daemon.
- **Cross-seam dependencies:** R00, R03A, R01B, R10.
- **Pilot relevance:** mandatory. A provider comparison is invalid if fork and upstream binaries are not identified and reload-safe.
- **Review:** `full`, rank 2, score 16/16. Divergence 4, operations 4, contested authority 4, pilot 4.
- **Cheapest decisive checks:** run the existing binary version/selfdev status and build-support tests in disposable environments; reproduce the incident's “already newest” versus mapped executable observation without changing the live daemon.
- **Confidence:** high that the responsibility is distinct and pilot-critical; medium on current incident reproduction.
- **Not checked:** no live daemon test or full build; no proof that later code closes the external incident.

### R01B - Reload handoff and client continuity (split from R01, linked to R04)

- **Owns:** reload notifications, interruption classification, reconnect handoff, and continuation metadata once a reload is authorized.
- **Excludes:** choosing the binary (R01A), wire compatibility policy (R03A), general session resume/supervision (R04), and persistence format (R06A).
- **Protected invariants:** all live clients observe reload; in-flight streams terminate without hanging; successor sessions receive notice; reload marker is active during reconnect; interrupted sessions can continue.
- **Evidence surfaces:** `tests/e2e/reload_multiclient.rs:84-336`; `tests/e2e/provider_behavior.rs:496-561`; `crates/jcode-selfdev-types/src/lib.rs:19-23`; `crates/jcode-app-core/src/catchup.rs:286-300`.
- **Operational risk:** High, but its observable contracts are already directly tested.
- **Cross-seam dependencies:** R01A, R03A, R04, R06A.
- **Pilot relevance:** conditional. Required only if the provider pilot exercises reload or handoff.
- **Review:** `light`, score 13/16. High divergence/operations, but narrower invariant and strong tests. Escalate if R01A or R03A changes handoff semantics.
- **Cheapest decisive checks:** run the four reload multi-client tests and binary handoff tests; compare event ordering and terminal states across refs.
- **Confidence:** medium-high.
- **Not checked:** no live binary run; tests were inspected, not all executed.

### R02 - Configuration provenance, provider resolution, auth, and routing (renamed, retained as one pilot seam)

- **Owns:** configuration layers/provenance, provider/account credentials as references rather than secret values, model/provider selection, route/failover resolution, sidecar configuration, and the resulting request-provider identity.
- **Excludes:** wire handshake (R03A), generic daemon build identity (R01A), durable transcript persistence (R06A), and tool/MCP lifecycle (R07A).
- **Protected invariants:** config provenance is explainable; explicit provider/model choice beats stale ambient state; credentials are not exposed; route selection is deterministic; model switch resets incompatible provider session state; sidecar failures do not silently route to an unintended provider.
- **Evidence surfaces:** `crates/jcode-base/src/config.rs:265,419-603`; `crates/jcode-base/src/provider/mod.rs:98-155,286,343-446`; `crates/jcode-base/src/provider/catalog_routes.rs:23-1153`; `crates/jcode-config-types/src/lib.rs:372-474,632-657,1260-1300`; `src/cli/provider_init.rs:375-433,1174-1210`; `tests/provider_matrix.rs:476-608,855-901`; `tests/e2e/provider_behavior.rs:680-834`; fork incident summary `PRESCREEN.md:117` and ancestor `a69ef9710`.
- **Operational risk:** Very high. It combines the leading divergence with credential/account state and is the likely pilot.
- **Cross-seam dependencies:** R00, R01A, R03A, R06A, R07A. R01B only if reload is in scope.
- **Pilot relevance:** mandatory and primary pilot candidate.
- **Review:** `full`, rank 3, score 16/16. Divergence 4, operations 4, contested authority 4, pilot 4.
- **Cheapest decisive checks:** use a deterministic mock provider and one non-secret model route; compare config provenance, selected route, auth readiness, provider session reset, and emitted wire metadata on fork/upstream refs.
- **Confidence:** high on coherence for a bounded provider pilot, medium on final authority.
- **Not checked:** no real credentials, network provider, or full provider matrix; no claim that upstream is better.

### R03A - Wire compatibility and build/protocol handshake (split from R03)

- **Owns:** version/build identity carried on subscribe, compatibility verdicts, protocol versioning, wire schema stability, and reconnect-required decisions at attach time.
- **Excludes:** executable selection (R01A), reconnect execution/handoff (R01B/R03B), session semantics (R04), and provider route policy (R02).
- **Protected invariants:** legacy clients remain compatible; protocol mismatch requires reconnect; differing known build hashes require reconnect; unknown hashes do not create a false incompatibility; short/full hashes compare safely; wire types remain serde-stable.
- **Evidence surfaces:** `crates/jcode-protocol/src/wire.rs:31-124` (`HandshakeCompatibility::evaluate`); `TaskGraphNodeSpec` at `:10-25` demonstrates explicit stable wire type; `tests/e2e/transport.rs:4` and `tests/e2e/binary_integration.rs:113-139`; protocol/reload diff stat from `git diff --stat base..HEAD` versus upstream.
- **Operational risk:** High. A false compatible attach can mix incompatible code; a false reconnect breaks clients.
- **Cross-seam dependencies:** R00, R01A, R01B, R02, R03B.
- **Pilot relevance:** mandatory prerequisite because provider behavior cannot be compared across mismatched runtime identities.
- **Review:** `full`, rank 6, score 13/16. Divergence 3, operations 4, contested authority 3, pilot 3. It replaces the broad R03 seed as the coherent full seam.
- **Cheapest decisive checks:** run compatibility unit tests for legacy, protocol mismatch, same hash, short/full hash, and unknown hash; compare handshake event traces on both refs without replay.
- **Confidence:** high on contract, medium on fork/upstream semantic equivalence.
- **Not checked:** no exhaustive wire compatibility matrix or mobile/iOS client behavior.

### R03B - Transport and client lifecycle adaptation (split from R03)

- **Owns:** Unix/WebSocket transport behavior, client attach/takeover/disconnect lifecycle, reconnect mechanics after a compatibility verdict.
- **Excludes:** verdict policy (R03A), session business state (R04), build choice (R01A), and persistence (R06A).
- **Protected invariants:** transport variants expose equivalent subscribe/history/message/resume behavior; takeover does not duplicate live mappings; disconnect cleanup is idempotent.
- **Evidence surfaces:** `tests/e2e/transport.rs:4`; `tests/e2e/burst_spawn.rs:183-655`; `crates/jcode-app-core/src/server/client_lifecycle.rs`; `crates/jcode-app-core/src/server/client_lifecycle_tests/`.
- **Operational risk:** High but bounded by existing lifecycle tests.
- **Cross-seam dependencies:** R03A, R04, R06A.
- **Pilot relevance:** conditional, only if the pilot compares transport or reconnect.
- **Review:** `light`, score 12/16. Escalate if handshake changes or lifecycle tests disagree.
- **Cheapest decisive checks:** run transport and takeover/resume tests and compare event/order invariants.
- **Confidence:** medium.
- **Not checked:** no Windows/mobile live transport matrix.

### R04 - Session lifecycle, supervision, recovery, backoff, and shutdown (retained but narrowed)

- **Owns:** session create/attach/resume/cancel/shutdown, supervision and recovery of session processes, retry/backoff, interruption status, and session-level liveness.
- **Excludes:** binary selection (R01A), wire verdict (R03A), swarm task assignment (R05A/B), and durable evidence serialization (R06A).
- **Protected invariants:** resume restores intended model/history metadata; interrupted work is explicit; cancellation/shutdown terminates without hangs; backoff prevents hot loops; one session cannot steal another's live state.
- **Evidence surfaces:** `tests/e2e/provider_behavior.rs:155-561,682-834`; `tests/e2e/session_flow.rs:4-237`; `tests/e2e/burst_spawn.rs`; `crates/jcode-app-core/src/server/client_session.rs` and `reload_recovery.rs`.
- **Operational risk:** High; direct provider and reload interactions make it a dependency seam.
- **Cross-seam dependencies:** R01A/B, R03A/B, R05B, R06A.
- **Pilot relevance:** light prerequisite only for a pilot that includes resume or reload; otherwise acceptance smoke check.
- **Review:** `light`, score 12/16. The seed is coherent after excluding wire and swarm dispatch, but not top-six because the proposed provider pilot can avoid supervision changes.
- **Cheapest decisive checks:** run resume, cancellation, reload interruption, and timeout tests; inspect backoff counters and terminal states.
- **Confidence:** medium-high.
- **Not checked:** no live process crash/restart campaign.

### R05A - Swarm plan/DAG semantics and control-plane state (split from R05)

- **Owns:** plan/node schema, dependency readiness, assignment state transitions, control-log event vocabulary/fold, replay, artifact evidence, and coordinator/task-control semantics.
- **Excludes:** worker process spawn mode, retry/backoff, terminal/session supervision (R05B/R04), and UI rendering.
- **Protected invariants:** replay is idempotent; only valid dependencies become runnable; control state is derivable from append-only events; completed status without artifact is not evidence; old log variants remain replayable.
- **Evidence surfaces:** `crates/jcode-plan/src/dag/tests.rs:45-99,613-669`; `crates/jcode-plan/src/lib.rs:556-640`; `crates/jcode-swarm-core/src/control_log.rs:1-21,32-136`; `crates/jcode-swarm-core/tests/control_log_properties.rs`; `crates/jcode-app-core/src/server/comm_control_tests/control_log_*`.
- **Operational risk:** High but primarily data/semantic rather than current pilot risk.
- **Cross-seam dependencies:** R00, R05B, R06A, R03A.
- **Pilot relevance:** not required unless pilot is swarm-driven.
- **Review:** `light`, score 13/16. Large fork-only control-log addition is contested, but its invariants are separately testable and the incident is in dispatch/liveness.
- **Cheapest decisive checks:** run DAG replay/idempotence and control-log property tests; compare event fold against server state for a small plan.
- **Confidence:** medium.
- **Not checked:** no multi-host merge or long-run log migration test.

### R05B - Worker dispatch, spawn mode, liveness, reclaim, and failure backoff (split from R05)

- **Owns:** run-plan assignment dispatch, worker spawn mode, dead-worker detection, failure scoreboard, reclaims, retry limits, and bounded backoff.
- **Excludes:** plan graph truth/control-log schema (R05A), generic session lifecycle (R04), and TUI display (R08B).
- **Protected invariants:** a dead worker cannot cause unbounded reassignment/spawn storms; explicit headless versus terminal spawn authority is honored; assignments eventually become completed/failed/blocked with evidence; reclaim is bounded and preserves history; failures are observable.
- **Evidence surfaces:** incident summary `PRESCREEN.md:117-119` with hash `7fdd9040...`; `crates/jcode-plan/src/lib.rs:581-640` (`MAX_DEAD_ASSIGNEE_RECLAIMS`, stranded reclaim); `scripts/test_swarm.py:86-428`; `crates/jcode-app-core/src/server/comm_control_tests/failure_scoreboard.rs`, `dag_e2e.rs`; `crates/jcode-app-core/src/server/swarm.rs`.
- **Operational risk:** Very high. The six-node incident completed zero nodes, emitted ~76 assignments, and created 190 session files.
- **Cross-seam dependencies:** R00, R04, R05A, R06A, R08A only for operator controls.
- **Pilot relevance:** not required for provider-only pilot; mandatory for any swarm-driven pilot.
- **Review:** `full`, rank 5, score 15/16. Divergence 4, operations 4, contested 4, pilot 3.
- **Cheapest decisive checks:** deterministic six-node mock run with terminal/headless spawn modes; assert bounded assignments, session-file growth, reclaim count, and terminal outcomes without real agents.
- **Confidence:** high on incident and invariant; medium on current fix completeness.
- **Not checked:** no live six-node rerun; no external terminal-backed worker reproduction.

### R06A - Durable session evidence, journals, snapshots, and replay (split from R06)

- **Owns:** session persistence format, history/evidence records, journals, snapshot/replay semantics, provenance, and recovery of durable session state.
- **Excludes:** memory graph/ranking and backup retention (R06B), live session supervision (R04), and swarm control-log authority (R05A).
- **Protected invariants:** persisted history resumes without loss or duplication; evidence carries session/parent/child identity; replay is deterministic and bounded; corrupt/partial writes do not silently fabricate state.
- **Evidence surfaces:** `crates/jcode-session-types/src/evidence.rs:1-27,157-327`; `crates/jcode-app-core/src/replay.rs:120-401`; `tests/e2e/provider_behavior.rs:155-398`; `crates/jcode-app-core/src/server/swarm_persistence.rs` and tests.
- **Operational risk:** High because state loss is user-visible, but current pilot can avoid migration.
- **Cross-seam dependencies:** R04, R05A/B, R01B.
- **Pilot relevance:** light prerequisite for any resume assertion; otherwise defer.
- **Review:** `light`, score 11/16. Smaller overlap and no quantified current incident in the provided records.
- **Cheapest decisive checks:** round-trip persisted session/evidence fixtures; replay interrupted history and assert event/message counts.
- **Confidence:** medium.
- **Not checked:** no corruption/fuzz or cross-version migration campaign.

### R06B - Memory, backup, and long-lived recall policy (split from R06)

- **Owns:** memory graph/storage, backup/snapshot retention, recall/rerank provenance, and memory-specific replay/benchmarks.
- **Excludes:** session transcript correctness (R06A), swarm control logs (R05A), and provider prompt policy (R02).
- **Protected invariants:** recall provenance is retained; backup failures do not erase active state; memory scope is isolated; replay/recall remains deterministic enough for benchmarks.
- **Evidence surfaces:** `crates/jcode-memory-types/src/graph/graph_tests.rs`; `crates/jcode-base/src/memory.rs`, `memory_agent.rs`, `memory_rerank.rs`; `src/bin/memory_recall_bench.rs`, `src/bin/session_memory_bench.rs`; `scripts/jcode_memory_snapshot.py`, `scripts/memory_regression_gate.sh`.
- **Operational risk:** Medium-high, but not pilot-critical.
- **Cross-seam dependencies:** R06A, R02, R07A.
- **Pilot relevance:** defer unless the selected provider path changes memory context.
- **Review:** `defer`, score 9/16. The seed's persistence/memory mix was incoherent; no Phase 0 incident requires it.
- **Cheapest decisive checks:** run graph tests and memory regression gate on fixed fixtures; escalate if provider pilot uses memory context.
- **Confidence:** medium-low until storage authority is inspected.
- **Not checked:** backup restore, external graph service, and large-memory behavior.

### R07A - Tool execution and MCP lifecycle/schema authority (split from R07)

- **Owns:** tool registry/dispatch, MCP server pool, connection/reconnect lifecycle, schema advertisement/cache, consent gating at execution, and per-session handle ownership.
- **Excludes:** generic discovery catalog, telemetry/analytics, network transport policy, and provider routing.
- **Protected invariants:** disabled MCP servers are not auto-spawned; shared pool avoids N x M process growth; failed connects cool down; cached schemas are config-fingerprint-bound hints, never truth; tool calls wait for live connection; consent is enforced before side effects.
- **Evidence surfaces:** `crates/jcode-base/src/mcp/pool.rs:1-45,65-110,144-180,330-394`; `crates/jcode-base/src/mcp/schema_cache.rs:1-23,33-63,90-155`; `crates/jcode-base/src/mcp/schema_cache_tests.rs:76-150`; `crates/jcode-tool-core/src/lib.rs:14-80`; `tests/e2e/safety.rs`.
- **Operational risk:** High, but no Phase 0 quantified incident.
- **Cross-seam dependencies:** R02, R04, R06A, R08A.
- **Pilot relevance:** only if provider route uses tools/MCP.
- **Review:** `light`, score 11/16. The split makes the behavior coherent, but no current pilot dependency.
- **Cheapest decisive checks:** run MCP schema cache tests and a disabled/failed-connect/shared-pool fixture; escalate on provider-pilot tool use.
- **Confidence:** medium-high for boundaries.
- **Not checked:** real MCP servers, browser/computer consent, or telemetry side effects.

### R07B - Discovery, telemetry, network, and consent policy (split from R07)

- **Owns:** tool/provider discovery surfaces, network/browser/computer policy, telemetry/reporting, analytics consent, and externally visible capability declarations.
- **Excludes:** actual tool/MCP process execution (R07A), provider route selection (R02), and UI rendering (R08B).
- **Protected invariants:** discovery does not require credentials; consent precedes network or analytics side effects; telemetry is opt-in/appropriately scoped; reported capabilities match executable capabilities.
- **Evidence surfaces:** `scripts/test_openrelay_discovery_test.py:51-76`; `scripts/test_benchmark_discovery.py:18-103`; `crates/jcode-base/src/browser.rs`, `mcp/protocol.rs`, telemetry crates; `scripts/security_preflight.sh`.
- **Operational risk:** Medium-high due external side effects.
- **Cross-seam dependencies:** R02, R07A, R08A.
- **Pilot relevance:** defer for a credential-free provider route; escalate if discovery or network is exercised.
- **Review:** `defer`, score 10/16.
- **Cheapest decisive checks:** run credential-free discovery tests and static security preflight; inspect network/telemetry call sites.
- **Confidence:** medium-low.
- **Not checked:** live network, analytics backend, or mobile/browser integration.

### R08A - Input and command semantics (split from R08)

- **Owns:** CLI argument/command parsing, keymaps, operator command semantics, interrupt/cancel commands, and command-to-server request mapping.
- **Excludes:** rendering, session picker presentation, desktop window adaptation, and backend command authority.
- **Protected invariants:** commands are deterministic and backward-compatible; dangerous actions require intended consent; soft/urgent interrupt semantics are preserved; CLI/TUI command mapping does not silently alter backend intent.
- **Evidence surfaces:** `src/cli/args.rs`, `src/cli/commands.rs`, `src/cli/dispatch.rs`, `src/cli/commands_tests.rs`, `scripts/test_soft_interrupt.py:144-542`, `scripts/verify_alt_shift_e_reaches_terminal.sh`.
- **Operational risk:** Medium-high.
- **Cross-seam dependencies:** R03A/B, R04, R08B.
- **Pilot relevance:** only operator smoke test.
- **Review:** `light`, score 10/16; split required, not full.
- **Cheapest decisive checks:** CLI dispatch tests and soft-interrupt suite.
- **Confidence:** medium.
- **Not checked:** full keyboard matrix or accessibility behavior.

### R08B - TUI render state and operator feedback (split from R08)

- **Owns:** render model, cards/tiles, status and error presentation, layout, and observable operator feedback for backend state.
- **Excludes:** input semantics (R08A), session selection domain model (R08C), desktop adaptation (R08D), and backend truth.
- **Protected invariants:** rendered status reflects backend state without inventing transitions; errors and progress remain visible; rendering does not mutate control state; output remains stable under large swarm/session data.
- **Evidence surfaces:** `crates/jcode-tui-render/src/lib.rs`, `swarm_tiles.rs`, `memory_tiles.rs`; `crates/jcode-tui/src/tui/info_widget*.rs`; `crates/jcode-tui-render/tests/swarm_gallery_fuzz_audit.rs`; `scripts/widget_quality.py`.
- **Operational risk:** Medium-high, but broad and non-pilot.
- **Cross-seam dependencies:** R05A/B, R06A/B, R08A.
- **Pilot relevance:** defer to smoke evidence.
- **Review:** `defer`, score 9/16.
- **Cheapest decisive checks:** existing render/fuzz/widget quality checks; escalate on visible state mismatch.
- **Confidence:** medium-low.
- **Not checked:** debug-socket frames or desktop visual runs.

### R08C - Session picker and selection semantics (split from R08)

- **Owns:** session list filtering, metadata display, resume/create selection, and picker-specific actions.
- **Excludes:** generic rendering primitives (R08B), backend session lifecycle (R04), and command parser (R08A).
- **Protected invariants:** picker selection targets the displayed session; metadata is not mistaken for history; resume action is explicit and reversible; filtering does not hide active/recoverable sessions unintentionally.
- **Evidence surfaces:** `crates/jcode-tui-session-picker/src/lib.rs:105`; `crates/jcode-config-types/src/lib.rs:46-62`; `tests/e2e/provider_behavior.rs:256-398`; session picker references under `crates/jcode-tui`.
- **Operational risk:** Medium.
- **Cross-seam dependencies:** R04, R06A, R08B.
- **Pilot relevance:** not required for a noninteractive provider pilot.
- **Review:** `defer`, score 8/16.
- **Cheapest decisive checks:** picker unit tests plus resume target fixture.
- **Confidence:** medium-low.
- **Not checked:** interactive TUI session-picking frames.

### R08D - Desktop/mobile adaptation and platform shells (split from R08)

- **Owns:** desktop window adaptation, mobile/iOS/web shells, platform launch and rendering adapters.
- **Excludes:** shared backend protocol, generic TUI render state, and command semantics.
- **Protected invariants:** platform adapters preserve shared protocol/session contracts; platform-only failures do not alter core state; launch/update identity is reported.
- **Evidence surfaces:** `crates/jcode-desktop`, `.github/workflows/ios-testflight.yml`, `tests/e2e/windows_lifecycle.rs`, `scripts/desktop_journey_e2e.sh`, `scripts/check_web_mobile.sh`.
- **Operational risk:** Medium, externally broad.
- **Cross-seam dependencies:** R01A, R03A/B, R08B, R10.
- **Pilot relevance:** defer.
- **Review:** `defer`, score 8/16.
- **Cheapest decisive checks:** platform compile/workflow validation and one adapter smoke test.
- **Confidence:** low-medium.
- **Not checked:** desktop/mobile runtime or CI artifact behavior.

### R09A - Quality-gate classifier semantics and workflow execution (split from R09)

- **Owns:** production/test source classification, parser semantics, adversarial fixtures, and ordering of classifier tests before panic/swallowed-error gates.
- **Excludes:** size debt, ratchet policy, ownership attribution, and behavioral seam cleanup.
- **Protected invariants:** test-only Rust is excluded accurately; production code is not undercounted; parser handles comments/strings/raw strings/multiline cfg; shared implementation is used by both gates; current red behavior is not hidden.
- **Evidence surfaces:** `QUALITY_GATES.md:33-57,73-88,90-106`; `scripts/rust_production_filter.py`; `tests/test_rust_production_filter.py:23-278`; integrated commits `fb1168a6a`, `0508e3f7b`, `f9c70d1be`.
- **Operational risk:** High historically, now bounded by 17 passing adversarial tests.
- **Cross-seam dependencies:** R00 and R09B; all behavior seams rely on its truth.
- **Pilot relevance:** trusted-baseline prerequisite.
- **Review:** `light`, score 12/16. It was already independently repaired and approved; escalate only on parser regression.
- **Cheapest decisive checks:** rerun the 17 tests, py_compile, and both gate scripts without `--update`.
- **Confidence:** high.
- **Not checked:** no new language/parser constructs beyond current adversarial suite.

### R09B - Debt attribution, ratchet policy, CI quality budgets, and inherited-red handling (split from R09)

- **Owns:** classification of current size/panic/swallowed debt, baseline versus inherited/current/fork-owned attribution, budget policy, and CI interpretation without blanket rebaseline.
- **Excludes:** parser implementation (R09A), responsibility behavior fixes, and synchronization mechanics (R00).
- **Protected invariants:** current red debt remains visible; no `--update` without ownership decision; parser correction is not confused with baseline tightening; inherited curated debt is separated from fork drift; green gates remain trusted.
- **Evidence surfaces:** `QUALITY_GATES.md:19-31,46-71,108-124`; `BASELINES.md:181-194,245-265`; audit incident hash `a672073e...`; `scripts/check_code_size_budget.py`, `check_test_size_budget.py`, `check_panic_budget.py`, `check_swallowed_error_budget.py`; current reported 60/31 size violations, panic 46 vs 31, swallowed 3077 vs 2987.
- **Operational risk:** Very high because every remediation and pilot regression budget depends on trustworthy attribution.
- **Cross-seam dependencies:** R00, R01A, R02, R03A, R05B; all full seams.
- **Pilot relevance:** mandatory prerequisite. Pilot must use trusted tests and preserve inherited red policy.
- **Review:** `full`, rank 4, score 15/16. Divergence 4, operations 4, contested attribution 3, pilot 4.
- **Cheapest decisive checks:** reproduce all gate results without update; replay original baseline with repaired scripts; attribute pilot diff by isolated slice and rerun only affected gates.
- **Confidence:** high for current gate interpretation, medium for per-seam ownership of size debt.
- **Not checked:** complete file-by-file attribution of all 60/31 size violations; no remediation.

### R10 - Packaging, release, update, and distribution authority (retained but narrowed)

- **Owns:** Nix/flake package outputs, install/update channels, release metadata, wrappers/launchers, and distribution artifacts.
- **Excludes:** live daemon target choice (R01A), protocol identity (R03A), and repository sync governance (R00).
- **Protected invariants:** published version/hash is the one launched; update refuses unsafe divergence; release metadata matches binary; package installs are reversible and do not mutate user state unexpectedly.
- **Evidence surfaces:** `crates/jcode-update-core/src/lib.rs:16,59-133,279-306,335-437`; `flake.nix`, `nix/`, `.github/workflows/release.yml`, `scripts/install*.sh`, `scripts/quick-release.sh`; incident summary in `PRESCREEN.md:118`.
- **Operational risk:** Medium-high, but authority is downstream of R01A.
- **Cross-seam dependencies:** R00, R01A, R03A, R10 itself.
- **Pilot relevance:** only as a build acquisition/identity smoke check.
- **Review:** `light`, score 9/16. Narrower shared code surface and no pilot need beyond identity.
- **Cheapest decisive checks:** update-core unit tests, package metadata comparison, and a non-mutating launcher/version check.
- **Confidence:** medium.
- **Not checked:** release publication, remote artifacts, or Nix evaluation across platforms.

### R11 - Documentation, incidents, and backlog governance (retained, narrowed)

- **Owns:** active recovery docs, incident summaries/hashes, maintenance queue truth, stale instruction retirement, and provenance of operational claims.
- **Excludes:** code ownership, gate verdicts, and runtime behavior.
- **Protected invariants:** docs do not claim unresolved work is open; incident evidence includes absolute path/hash/summary; old snapshots are append-only; recommendations remain distinguishable from facts.
- **Evidence surfaces:** `BASELINES.md:106-113,226-243`; `PRESCREEN.md:110-132`; `PROGRESS.md:1-49`; maintenance reconciliation and ancestor checks; existing dirty prompt preserved.
- **Operational risk:** Medium. Incorrect docs can cause unsafe replay, but the responsibility is governance rather than runtime authority.
- **Cross-seam dependencies:** R00 and every seam's ledger; especially R09B.
- **Pilot relevance:** documentation prerequisite, not a behavioral pilot seam.
- **Review:** `light`, score 10/16. The stale-state incident is resolved enough for light review; escalate when a ledger relies on external notes.
- **Cheapest decisive checks:** verify links, hashes, ancestry of cited fixes, and append-only snapshots.
- **Confidence:** high for current negative findings, medium for external-note completeness.
- **Not checked:** no independent hash recomputation of every maintenance note; no docs edits made.

## Full-review ranking and mode allocation

| Rank | ID | Score (D/O/C/P out of 4) | Mode | Why this depth |
|---:|---|---:|---|---|
| 1 | R00 | 16 (4/4/4/4) | full | Governs every later provenance and sync claim. |
| 2 | R01A | 16 (4/4/4/4) | full | Concrete stale-daemon incident and pilot-critical authority. |
| 3 | R02 | 16 (4/4/4/4) | full | Leading two-sided divergence and likely pilot seam with credential risk. |
| 4 | R09B | 15 (4/4/3/4) | full | Trusted regression budget and inherited-red attribution gate all work. |
| 5 | R05B | 15 (4/4/4/3) | full | Quantified spawn storm and liveness/backoff invariant. |
| 6 | R03A | 13 (3/4/3/3) | full | Build/protocol attach safety is a pilot prerequisite despite narrower scope. |
| 7 | R01B | 13 (3/4/3/3) | light | Directly tested, escalate if reload is in pilot. |
| 8 | R05A | 13 (4/3/3/3) | light | Large control-plane divergence but not provider-pilot critical. |
| 9 | R04 | 12 (3/4/3/2) | light | Strong runtime risk, but avoid changing it in provider-only pilot. |
| 10 | R03B | 12 (3/4/3/2) | light | Lifecycle/transport is separable and test-rich. |
| 11 | R09A | 12 (3/4/2/3) | light | Repaired and independently approved; preserve current semantics. |
| 12 | R06A | 11 (3/3/3/2) | light | Resume evidence matters, but no migration pilot yet. |
| 13 | R07A | 11 (3/3/3/2) | light | MCP/tool authority is coherent but not current pilot scope. |
| 14 | R07B | 10 (3/3/3/1) | defer | Discovery/network/telemetry requires separate external checks. |
| 15 | R11 | 10 (3/2/3/2) | light | Governance is low-risk after Phase 0 reconciliation. |
| 16 | R06B | 9 (3/3/3/0) | defer | Memory/backup is not needed for a minimal provider pilot. |
| 17 | R08A | 10 (4/3/2/1) | light | Command semantics are testable but not authority for pilot. |
| 18 | R08B | 9 (4/3/2/0) | defer | Broad visual surface, no current pilot dependency. |
| 19 | R08C | 8 (4/2/2/0) | defer | Picker can be bypassed in a deterministic pilot. |
| 20 | R08D | 8 (3/3/2/0) | defer | Platform adapters are downstream and broad. |
| 21 | R10 | 9 (2/3/2/2) | light | Build/package smoke only; do not conflate with daemon authority. |

The six full seams are intentionally not the original broad R03/R05/R09 rows. The split preserves the cap while putting deep review on the actual protected invariants and incident-bearing authorities.

## Pilot prerequisites and smallest safe pilot question

### Must pass before a bounded pilot

1. **R00 full ledger:** fixed fork/upstream/base refs, no broad replay, explicit semantic-equivalence assumptions, and rollback/stop budgets.
2. **R01A full ledger:** deterministic build/source identity and a non-mutating check that the selected executable is the one reported/launched.
3. **R02 full ledger:** one credential-free or fixture-backed provider/model route with provenance and auth-state invariants defined.
4. **R03A full ledger:** handshake compatibility verdict and reconnect behavior are known for the selected fork/upstream binaries.
5. **R09A light ledger plus R09B full ledger:** 17 classifier tests pass; trusted green gates remain green; red gates remain red and attributed; no blanket ratchet update.
6. **R06A light smoke condition:** if the pilot observes session continuity, a persisted history round-trip must pass. R01B/R04 are required only if reload/resume is part of the pilot question.
7. **Pilot does not require R05B, R06B, R07B, or R08 seams** unless the selected stack exercises swarm, memory, external discovery, or UI/platform behavior.

### Smallest safe pilot question (question only, not implementation)

> On disposable, fixed fork and upstream refs, does one non-secret provider/model route resolve from the same declared configuration provenance to the intended provider and wire identity, while preserving the selected session's observable request/result and leaving trusted quality-gate results unchanged, without exceeding the declared conflict, semantic-rewrite, time, or rollback budgets?

This is deliberately narrower than “can the fork be replayed?” It tests the high-value R02 seam while exercising R00, R01A, R03A, and R09. It must use fixtures or a local mock, not real credentials or external publication. If it needs swarm, MCP, memory, UI, or live reload to answer, the pilot is no longer minimal and the corresponding light/deferred seam must be escalated before proceeding.

## Negative findings and explicit gaps

- No shared stable patch-ID cluster was found. Do not call any seam patch-equivalent.
- No current upstream authority was inferred from path overlap, commit subject, or `vendor/upstream`; the latter is still merge base.
- No broad Rust build/test suite, live daemon incident reproduction, real-provider/network test, mobile/desktop runtime test, or full semantic comparison of curated-sync contents was performed.
- The 127 fork and 64 upstream unclassified paths remain unassigned. They may contain missing responsibilities or incidental files.
- The size-ratchet violations are structurally real, but ownership of every violating file was not attributed. The parser correction and stale swallowed baseline tightening remain distinct.
- The current live daemon stale-build failure mode is unknown without an operational reproduction.
- The three external incident notes were used only through the hashed summaries in `PRESCREEN.md`; this map did not read the forbidden critic artifact or expand into external-note reanalysis.
- R07B, R06B, R08B/C/D, R10, and parts of R03B remain low-to-medium confidence until their targeted checks run. Low confidence must not pass a later gate.

## Concise proposed RESPONSIBILITIES table

| ID | Proposed responsibility | Owns | Excludes | Mode |
|---|---|---|---|---|
| R00 | Integration provenance and sync governance | refs, ancestry, curated-sync and equivalence evidence | runtime behavior | full |
| R01A | Build identity and daemon reload authority | executable/source identity, reload target selection | handoff/session continuity | full |
| R01B | Reload handoff and client continuity | notifications, interruption, reconnect continuation | binary choice | light |
| R02 | Configuration provenance, provider resolution, auth, routing | config/provider/account/model/route outcome | wire and persistence | full |
| R03A | Wire compatibility and build/protocol handshake | attach verdict and stable wire identity | transport execution | full |
| R03B | Transport and client lifecycle adaptation | attach/takeover/disconnect mechanics | verdict policy | light |
| R04 | Session lifecycle and supervision | resume/cancel/shutdown/recovery/backoff | swarm dispatch and persistence format | light |
| R05A | Swarm plan/DAG and control-plane state | graph semantics, event fold, artifact evidence | worker spawn/liveness | light |
| R05B | Worker dispatch, spawn mode, liveness, reclaim | assignments, backoff, dead-worker containment | graph truth | full |
| R06A | Durable session evidence and replay | journals, history, snapshots, provenance | memory graph | light |
| R06B | Memory, backup, and recall policy | memory/backup/rerank | session transcript | defer |
| R07A | Tool execution and MCP lifecycle | pool, schema cache, tool-side consent | discovery/telemetry | light |
| R07B | Discovery, telemetry, network, consent policy | external capability/reporting policy | MCP process execution | defer |
| R08A | Input and command semantics | CLI/keymap/interrupt mapping | rendering | light |
| R08B | TUI render state and operator feedback | cards, tiles, status presentation | backend truth | defer |
| R08C | Session picker and selection semantics | picker filtering/resume selection | backend lifecycle | defer |
| R08D | Desktop/mobile/platform adaptation | platform shells and adapters | shared backend contracts | defer |
| R09A | Quality-gate classifier semantics | parser contract and adversarial tests | ratchet ownership | light |
| R09B | Debt attribution and ratchet policy | inherited-red handling and budget policy | parser code | full |
| R10 | Packaging, release, update, distribution | package/launcher/update artifacts | live daemon authority | light |
| R11 | Documentation, incidents, backlog governance | durable operational truth and maintenance status | code behavior | light |

## Top-six ranking

1. **R00** - Integration provenance and sync governance, 16/16.
2. **R01A** - Build identity and daemon reload authority, 16/16.
3. **R02** - Configuration/provider/auth/routing, 16/16.
4. **R09B** - Debt attribution and ratchet policy, 15/16.
5. **R05B** - Worker dispatch/spawn/liveness/backoff, 15/16.
6. **R03A** - Wire compatibility and build/protocol handshake, 13/16.

These are research-depth choices only. They do not select fork/upstream authority, prescribe remediation, or authorize a pilot implementation.
