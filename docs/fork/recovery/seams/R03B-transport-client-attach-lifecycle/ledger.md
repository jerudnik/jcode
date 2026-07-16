# R03B Transport and client attach lifecycle: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `light` |
| Research budget | `6 decisive checkpoints, exhausted without expansion` |
| Recommended disposition | `retain-fork` |
| Confidence | `medium-high` |

R03B owns Unix/WebSocket connection attachment, takeover, disconnect/reconnect mechanics, and idempotent live-mapping cleanup. It excludes R03A compatibility policy and verdicts, R01 identity meaning or target selection, and R04 session business state, process lifecycle, reload handoff, and terminal-state policy. The source tree was read-only. No live user daemon, credential, external application network call, stash/ref operation, destructive action, or publication was used.

## Six decisive checkpoints

| # | Finding | Evidence and deterministic reproduction | Consequence |
|---:|---|---|---|
| 1 | The fixed comparison inputs resolve and their merge base is the recorded base. | `git merge-base 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b` returned `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`. | All authority findings are bounded to reproducible refs under R00. |
| 2 | The scoped transport/attach surface exists on all three refs and is two-sided, so neither ancestry nor upstream provenance alone decides it. | Fixed-ref path inventory found `server/{socket,client_lifecycle,client_disconnect_cleanup,client_comm,client_api}.rs`, socket tests, and the three takeover/attach fixtures on base, upstream, and fork. | Authority had to be challenged semantically, not adopted mechanically. |
| 3 | The upstream live-attachment fanout repair is semantically present in the fork's curated sync, while fork-specific cleanup/mapping adaptation remains. | Upstream `684e27fe4` moves terminal `Done`/`Error` to `session_event_fanout_sender_with_fallback` and adds the live-attachment ordering fixture. `git show b3ed82a6b -- client_lifecycle.rs client_lifecycle_tests.rs` shows the same fanout/terminal ordering and fixture absorbed into the one-parent curated sync. Fixed-ref delta counts are fork `234+/198-` and upstream `80+/9-` in lifecycle, with fork-only changes in `client_comm.rs` and disconnect cleanup. | Retain the composed fork surface. This is not an exact patch-ID claim: R00 records `b3ed82a6b` as an ancestry gap, and the disposition is supported by symbol/test behavior instead. |
| 4 | Unix stale-socket recovery is fail-safe and fixture-backed without a daemon. | `socket.rs:48-115` never unlinks a refused socket from the client path and reaps only after no live listener, acquiring the daemon lock, and rechecking the listener. `socket_tests.rs:29-190` covers paired cleanup, refused-path preservation, stale-pair/lock reaping, and live-listener/held-lock non-removal. The isolated test command below is the required deterministic fixture; its first execution was deferred by compilation timeout before tests started. | A client cannot clean up a socket merely because its own connection was refused. |
| 5 | Attach mapping cleanup releases the connection record before slow destructive work and preserves a session with a live successor. | `client_lifecycle.rs:483-496` creates a per-connection entry; `client_disconnect_cleanup.rs:81-103` removes it, unregisters its sender, yields, then skips destructive cleanup when another connection for the session exists. `client_lifecycle.rs:2683` routes every lifecycle exit through that cleanup. Resume fixtures cover same-instance takeover, reconnect takeover with history, different-client attach, and multiple live attachment; `client_lifecycle_tests.rs:785-875` asserts ordered fanout to an attachment made mid-stream. | Reconnect/takeover can reclaim ephemeral mapping ownership without R03B deciding the session's business disposition. |
| 6 | The incident supports preserving the transport safety boundary, but does not transfer reload authority to R03B; no R03B-specific current R09 debt was found. | External incident `/Users/jrudnik/notes/projects/jcode/maintenance/bug-server-reload-stale-daemon-version-check.md`, SHA-256 `80012e2ce61c578c943263b944bbaca27ac7dbd440af50c1f21f6e0291d8f1a9`, documents a stale in-memory daemon whose on-disk channel appeared current. `RESPONSIBILITIES.md:63-70` assigns identity to R01 and ordered handoff to R04 then R03A. Searches of `QUALITY_GATES.md` and R09's ledger found no named `socket.rs`, `client_lifecycle.rs`, `client_disconnect_cleanup.rs`, `client_comm.rs`, or transport debt entry. | R03B may preserve a safe attachment path but must not choose a daemon target or claim reload success. Any future R03B source debt must remain attributed and red under R09, with no `--update`. |

## Authority and supported disposition

`retain-fork` is the one supported disposition. Upstream's post-base fanout repair is a useful candidate, not authority. The fork already carries its observable fanout and terminal ordering through the curated sync, then adds fork-specific connection bookkeeping and successor-safe cleanup. There is no demonstrated competing behavior that warrants `compose`, and no basis to delete or adopt an upstream file wholesale.

This conclusion is deliberately narrow. It does **not** bless every statement in the 3,134-line lifecycle dispatcher, session state transitions, or reload policy. Those remain R04/R01 territory and are not inferred from file co-location.

## Pilot relevance, fixture boundary, and cross-seam contract

- **Pilot relevance:** conditional, not a current pilot blocker. The approved smallest pilot does not require reload, resume, cancellation, or detached work (`RESPONSIBILITIES.md:72-88`), so it may use one fresh Unix attachment only. If it exercises reconnect/takeover, the fixture must retain the checkpoint-4 socket and checkpoint-5 attachment assertions.
- **Deterministic no-network fixture:** run with `JCODE_HOME=$(mktemp -d)` and a trap that removes only that directory:

  ```bash
  bash scripts/dev_cargo.sh test -p jcode-app-core --lib server::socket_tests:: -- --test-threads=1
  bash scripts/dev_cargo.sh test -p jcode-app-core --lib \
    server::client_lifecycle::tests::client_initiated_turn_fans_out_stream_and_terminal_events_to_live_attachments \
    -- --exact --test-threads=1
  ```

  These target temporary Unix paths, socket pairs, mock event streams, and local channels. They do not start or contact the user daemon and require no provider credential. A fresh `JCODE_HOME` Nix-build attempt was capped at 600 seconds and timed out while compiling `tokenizers`, before either selected test executed. On the warmed 2026-07-15 rerun, the socket filter passed `20 passed; 0 failed` in 0.44 seconds. The originally recorded fanout filter used the wrong generated module path and matched zero tests, so `dev_cargo.sh` correctly exited 97 rather than treating that as success. `cargo test -- --list` resolved the exact generated name shown above, and the corrected exact filter then passed `1 passed; 0 failed` in 0.93 seconds. The timeout and zero-match attempt remain explicit validation history, not test failures or passes.
- **Explicit defer boundary:** no Rust `WebSocket`, `websocket`, `web_socket`, or `ws://` transport implementation was found by fixed-ref source census. WebSocket/mobile attach has no deterministic owned fixture here. A WebSocket pilot, a new transport adapter, or a request to infer its mapping/cleanup semantics escalates to **full review before implementation**.
- **R03A:** R03A alone evaluates protocol/build compatibility after reconnect. R03B transports an already-decided event and must not reinterpret, suppress, or turn compatibility into identity truth.
- **R01 and R04:** R01 selects identity/target, R04 interrupts and hands off sessions/clients, then R03A evaluates reconnect compatibility. R03B supplies only safe connection cleanup/reattach mechanics and cannot report reload success.
- **R12/R06A:** live fanout is ephemeral delivery, not durable turn evidence. R12 owns terminal record emission and R06A persistence/replay. A successor attachment must not fabricate, duplicate, or delete either record.
- **R09:** no existing scoped debt entry was found. Before any source change, rerun the applicable trusted gates without `--update`, publish changed counts, and assign any red delta to R03B rather than hiding it in an overlay.

## Negative findings and gaps

- No exact stable patch-ID equivalence is claimed. The `b3ed82a6b` squash is an explicit R00 ancestry gap, so the finding is limited to the shown fanout semantics and fixture.
- No WebSocket transport surface or test was found at base, upstream, or fork. It is deferred, not silently treated as Unix-equivalent.
- No live daemon, real socket namespace, reload, client device, network, credential, or generic `server::Client` attach path was exercised.
- The checkpoint-5 resume fixtures establish intended attach/takeover outcomes, but this light review did not execute every resume fixture. R04 remains the owner of the resulting session state and terminal policy.
- The stale-daemon incident proves an identity/reload failure mode, not an R03B implementation defect. It is recorded only to constrain this seam's boundary.

## Acceptance, rollback, and escalation

- **Acceptance or retirement condition:** the isolated Unix socket and live-attachment fixtures now pass from a warmed cache with a disposable `JCODE_HOME`; retain this narrow attach/cleanup contract subject to coordinator review. The first fresh-build timeout and the corrected zero-match command remain recorded above. The seam becomes relevant to Phase 3 only when the selected fixture attaches to the server; it is otherwise a conditional seam record.
- **Rollback or stop condition:** do not modify the transport surface in this lane. Stop and restore the prior bounded plan, rather than broadening, if a change requires unlinking on client refusal, a live daemon, real credentials/network, a target-selection decision, session-state remediation, a R09 baseline update, or more than this six-checkpoint evidence budget.
- **Escalate to full review if:** authority becomes contested by a semantic conflict in the absorbed fanout behavior; a pilot exercises WebSocket/mobile attach, reload/resume/cancel/detached work, generic-client identity advertising, or a multi-client mapping leak; an attachment can reorder a terminal event, remove a live successor's mapping, or cause R12/R06A evidence duplication/loss; or the deterministic fixture fails.
- **Coordinator approval:** pending coordinator review; the isolated Unix fixtures now pass as recorded above.
- **Fable review:** pending independent Phase 4 architecture review.

## 2026-07-15 W0 approval amendment

Coordinator approval: **PASS as a conditional `retain-fork` light record**. The independent five-ledger review is [`../../reviews/2026-07-15-remaining-light-ledgers-opus-review.md`](../../reviews/2026-07-15-remaining-light-ledgers-opus-review.md), SHA-256 `b537bc5674fdb9385e60c2dd18a44db5e61ba4f57146cd57fbf91f7a58a8a55d`. Later isolated Unix socket fixtures passed 20/20 and the exact live-attachment fanout fixture passed 1/1 as recorded in `PROGRESS.md`.

The Phase 4 Fable/Opus architecture gate also passed, so the stale Fable-pending line is discharged by corrected Fable plan SHA-256 `b0bae9803fa726a489e0560fdc423daefa20bd8478ede0aa2772f7684ea21eb9` and independent plan review SHA-256 `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92`. This approves only the narrow Unix attach/cleanup contract. WebSocket/mobile attach and every listed escalation trigger remain deferred.
