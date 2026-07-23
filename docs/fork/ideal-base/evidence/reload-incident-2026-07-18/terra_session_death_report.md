# Reload session-loss investigation

**Scope.** Read-only investigation of the exec-handoff reload whose durable trace is
`~/.jcode/reload-traces/reload_1784425095420_8427524887491149517.jsonl`.
No repository files were modified, no build was run, and no reload was initiated by this investigation.

## Executive conclusion

The evidence does **not** show `session_skunk_1784421354092_7c818fd78fc6f1ae` dying during this reload. It shows that it was not a server-resident, attached, running session at reload time:

* its persisted snapshot is already `Closed`, has exactly one bootstrap/system message, no journal, `working_dir=/Users/jrudnik`, and an on-disk mtime of `2026-07-18 20:35:54 -0400`;
* the authoritative reload candidate snapshot at 21:38:15 contains `session_hedgehog_1784421350156_caa04b8547f40c54` as the only relevant attached/running peer, and contains **no skunk**;
* the reload trace records an intent for `hedgehog`, its checkpoint, and later history attachment and accepted-continuation delivery for `hedgehog`.

Therefore `skunk` neither failed to checkpoint nor failed successor rehydration. It was never eligible because it was not present in `swarm_members` with `status == "running"`. The session which actually survived and was rehydrated was `hedgehog`, via the later temporary client session `sauropod` resuming `hedgehog`.

This conflicts with the incident description’s name/cwd attribution (`skunk` described as an attached repo TUI). The persisted data says `skunk` is non-selfdev in `/Users/jrudnik`; the trace says the active peer was `hedgehog`, also in `/Users/jrudnik`. That discrepancy is itself the primary diagnosis: the UI/process identity named skunk was not the same as the server-side active agent/session identity used by the reload protocol.

## Evidence timeline

All log timestamps below are from `~/.jcode/logs/jcode-2026-07-18.log`; the snapshot contents use UTC metadata while filesystem mtime is local `-0400`.

| Time | Evidence | Consequence |
|---|---|---|
| 21:37:55.466–.501 | `hedgehog` completes a provider turn, persists, then runs a `bash` tool. A child `jcode starting` at .497 is consistent with the reported shell `jcode debug reload` invocation. | The server-side active agent is `hedgehog`, not `skunk`. |
| 21:38:15.420 | reload trace `signal_received`: daemon PID 63766, trigger `session_dolphin_1784425094817_a3bab5d9e1f8937a`. | The ephemeral `jcode server reload` client was correctly identified as trigger. |
| 21:38:15.429 | trace `candidate_snapshot`: `dolphin` is `ready`; `hedgehog` is `running`, non-headless; **skunk is absent**. | Candidate selection has no possible path to an intent for skunk. |
| 21:38:15.463 | trace `intent_skipped` for `dolphin`, `has_reload_ctx=false`, `triggering=true`. | Trigger got no directive, as expected for a non-selfdev transient CLI client. |
| 21:38:15.500 | trace and log: `intent_persisted` for `hedgehog`, role `interrupted_peer`; shutdown signal sent; trigger excluded from wait set; waits only for hedgehog. | Correctly preserves the one actual running peer. |
| 21:38:16.288–.289 | `hedgehog` changes `running -> ready`; server says all sessions checkpointed; daemon PID 63766 execs replacement binary. | Normal graceful drain/checkpoint of hedgehog. |
| 21:39:44.032–.045 | new client creates temporary `sauropod`; `SESSION_LIFECYCLE resume_start` says `source_session_id=sauropod`, `target_session_id=hedgehog`. | The post-reload client resumes hedgehog, not skunk. |
| 21:39:44.620 / .639 | reload trace says recovery intent was attached to hedgehog history, then delivered after matching continuation accepted. | Rehydration and recovery delivery succeeded for hedgehog. |
| 21:40 onward | `hedgehog` continues provider/tool activity and remains `Active`; its snapshot grows to 1,311 messages. | No server-agent crash occurred for the recovered session. |

## 1. What exec-handoff preserves and recovers

### A. Session conversation/state snapshot

Normal session persistence is durable under `~/.jcode/sessions/<session>.json` plus journal/backup files. The incident log shows frequent `SESSION_PERSISTENCE ... phase=save_done` entries for hedgehog before reload. The checkpoint protocol causes the active generation to finish/cancel and updates the swarm member status; it does not serialize the in-memory `sessions` map across `exec`.

The replacement process starts with an empty in-memory map. A reconnecting client creates a temporary source session and sends a resume request for its saved target session. In this incident, that target was hedgehog, not skunk. The `session-load`/persistence evidence after reconnection is thus expected to be for hedgehog.

Relevant source:

* `crates/jcode-app-core/src/server/reload.rs:140-160` persists recovery intents before initiating graceful shutdown.
* `crates/jcode-app-core/src/server/reload.rs:381-541` identifies active members, signals checkpoint, and waits for status transitions.
* `crates/jcode-app-core/src/server/client_session.rs:928+` handles resume. Runtime log at 21:39:44.045 explicitly identifies the `sauropod -> hedgehog` resume.

### B. Server-owned recovery intent

The durable intent is independent of the conversation snapshot and is stored at:

`~/.jcode/reload-recovery/<sanitized-session-id>.json`

An intent contains reload ID, session ID, role, `Pending`/`Delivered` status, a `ReloadRecoveryDirective`, reason, and timestamps.

* Storage schema and path: `crates/jcode-app-core/src/server/reload_recovery.rs:27-73`.
* `persist_intent`: `reload_recovery.rs:187-214` writes a `Pending` record.
* `pending_directive_for_session`: `reload_recovery.rs:234-` reads, but intentionally does not consume, it when composing history. Its comment at lines 237-241 explains why: a History response may be lost before the client sends the hidden continuation.
* `mark_delivered_if_matching_continuation` is the eventual acknowledgement mechanism in `reload_recovery.rs` following this section. The durable trace proves it was reached for hedgehog at 21:39:44.639.

### C. Eligibility

`persist_reload_recovery_intents` takes candidates only from `swarm_members` where `member.status == "running"` (`reload.rs:246-273`). It additionally inserts the triggering ID only if it was not already a candidate (`reload.rs:275-281`). For each candidate it obtains optional self-dev context and builds a directive. A non-triggering attached candidate receives `was_interrupted=true` through `is_headless || !is_triggering` (`reload.rs:286-294`), so it receives the generic interrupted-session directive even without a self-dev context. Roles are assigned at `reload.rs:317-330`.

`ReloadContext` is a different, optional per-session file, `~/.jcode/reload-context-<id>.json`. It is looked up without consuming it by `ReloadContext::peek_for_session` in `crates/jcode-app-core/src/tool/selfdev/reload.rs:18-62`. A context gives a tailored reconnect notice and continuation. Without it, a peer with `was_interrupted=true` gets the generic continuation message (`selfdev/reload.rs:127-162`).

## 2. Why skunk did not survive

### Direct answer

**Neither “never checkpointed” nor “successor failed to rehydrate” applies.** Skunk was not in the active server membership snapshot, therefore it did not enter the recovery/checkpoint pipeline at all.

The decisive chain is:

1. `reload.rs:268-272` filters candidates to `swarm_members` with status `running`.
2. The durable candidate trace at 21:38:15.429 has hedgehog `running`; skunk does not appear at any status.
3. Hence no `intent_persisted`, no shutdown signal, no wait-set membership, and no history recovery directive can exist for skunk.
4. The session file corroborates that this was not a live conversation: `skunk.json` is 1,270 bytes, one bootstrap message, status `Closed`, and no `.journal.jsonl` exists.
5. By contrast, hedgehog has a large journal/snapshot, is in the trace, receives `interrupted_peer`, reaches `ready` at the checkpoint, and later is resumed/delivered.

The log line quoted in the incident is correct but incomplete: the only wait-set member was `hedgehog` because that was the only `running` member. The trigger `dolphin` being excluded did not exclude skunk. Skunk was absent before that exclusion happened.

### Was a successor supposed to rehydrate skunk?

No successor attempted to do so. The reconnect source was `session_sauropod_1784425184033_e835b28b189671bb`, and the server log explicitly names target `session_hedgehog_1784421350156_caa04b8547f40c54`. The trace then confirms hedgehog recovery attachment and delivery. There is no skunk session-load, recovery-intent, or resume-target record because there is no skunk recovery operation.

### Important identity discrepancy

The phrase “TUI self-dev session in `/Users/jrudnik/labs/jcode`, session skunk” is disproved by the inspected persisted skunk file: it says `/Users/jrudnik`, `is_selfdev=false`. The reload trace’s active peer is `hedgehog`, with `swarm_id=/Users/jrudnik`, not the repo worktree. The supplied IDs and disk evidence therefore identify different logical sessions/processes. Any product fix should first make that mapping observable rather than assume the label visible in a terminal is the server agent ID.

## 3. Why `jcode debug reload` returned `Unknown session_id` before and after

### Resolution mechanism

This is not environment-variable lookup and it is not active-client selection for ordinary server debug commands.

The debug protocol carries an explicit optional `session_id` field (`crates/jcode-protocol/src/wire.rs:183-190`). The debug-socket tool merely forwards the optional caller-supplied field (`crates/jcode-app-core/src/tool/debug_socket.rs:74-100`, request construction at lines 105-136). There is no ambient `JCODE_SESSION` read in this path.

On the server, ordinary debug commands resolve their agent as follows:

1. If the request carried a session ID, use that exact ID.
2. Otherwise, use the server-global `session_id` if nonempty.
3. Otherwise, if exactly one in-memory agent exists, use it.
4. Else fail.

Source: `crates/jcode-app-core/src/server/debug_command_exec.rs:46-77`. The exact `Unknown session_id '<id>'` error comes only from the lookup at lines 59-65, after a target was selected but was missing from the current in-memory `sessions` map.

The global fallback is updated opportunistically when a normal client connection is set up (`crates/jcode-app-core/src/server/client_lifecycle.rs:562-565`). It is not a durable session attachment registry and is reset on exec. The distinct `ClientDebugState.active_id` mechanism in `server/debug.rs:37-95` only selects a TUI transport for the `client:` namespace. It does not resolve ordinary server-side `reload` commands.

### Incident conclusion

For both the pre- and post-reload failures, the error means the request targeted skunk or sauropod explicitly, or the server-global fallback held that ID, while its corresponding agent was not in the new/current `sessions` map. It does **not** mean the corresponding disk snapshot could not be loaded. The log does not contain the returned error text itself, so the exact request payload cannot be recovered from the file log; this conclusion follows directly from the only code path which emits the quoted error.

* Before reload, skunk had already been persisted `Closed` and was not in the active in-memory membership snapshot. A debug command directed to it correctly fails agent lookup even though the `.json` exists.
* After reload, sauropod was a temporary source client session created during reconnect. The actual resumed agent was hedgehog. A command directed to sauropod therefore correctly fails because it was not the recovered agent identity.

This also explains why `jcode debug sessions` showed only hedgehog: it reports the in-memory agent map, whereas `~/.jcode/sessions/` contains historical persisted sessions too.

## 4. Crash versus live TUI losing server-side session

The evidence supports **no crash of the logical active TUI/agent session**. It supports a normal server exec, connection break, then resume of hedgehog:

* The daemon PID 63766 successfully execs at 21:38:16.289. That necessarily drops client sockets and its in-memory agent map.
* The trace shows a clean hedgehog checkpoint (`running -> ready`) before exec, not a crash.
* A new client process/session (`sauropod`) connects at 21:39:44 and resumes hedgehog.
* The recovery directive is accepted by hedgehog, then hedgehog continues real provider/tool work through 21:40+.

The ephemeral trigger `dolphin` itself is persisted as `Crashed` with `Process 15783 exited unexpectedly (no shutdown signal captured)`. That is the short-lived CLI client process, not the daemon. Its state is unsurprising for a process whose purpose was to issue the reload and wait across the handoff. It is not evidence that hedgehog crashed.

The original visible TUI process may have remained alive while the daemon exec severed its socket, then reconnected under a fresh client/session identity. The server cannot preserve the old connection object across exec. What was lost was **the association between the visible client identity (`skunk`/later `sauropod`) and the actual server agent identity (`hedgehog`)**, not the hedgehog conversation state.

## Violated invariant

The narrower proposed invariant, “all non-triggering attached sessions must receive recovery intents,” is desirable but it is not exactly the failure evidenced here: the server did satisfy it for the only attached/running peer it knew about, hedgehog.

The violated end-to-end invariant is:

> Every attached TUI that can issue a session-targeted debug/self-dev command must have a durable, observable mapping to the server-side agent session that owns its conversation. Across exec handoff, the replacement must either resume that same agent ID and route session-targeted controls to it, or explicitly reject the control as a stale **client/session alias** with the resolved active target.

Current implementation violates that invariant because it treats the client-created source session (`sauropod`) and stale/closed alias (`skunk`) as ordinary agent IDs in debug routing. `resolve_debug_session` requires membership in an ephemeral in-memory agent map and has no client-to-resumed-agent resolution (`debug_command_exec.rs:46-77`).

## Minimal fix proposal (not implemented)

1. **Separate client identity from agent-session identity in control requests.** Maintain a durable-or-reconstructible `client_instance_id -> resumed_agent_session_id` binding during the reconnect/resume handshake. The already logged `client_instance_id`, source ID, and target ID demonstrate the data is available.
2. **Route debug/self-dev controls through that binding.** A debug command originating from an attached TUI should default to its resolved active agent session, not the temporary source session. An explicit historical session ID should remain supported, but resolve via an alias table when it is a known reconnect source.
3. **Make recovery eligibility reflect live attached connections, not only `swarm_members.status == "running"`.** Preserve an intent for every non-headless client with a live event sink/connection, even when the corresponding swarm member is `ready` or omitted. This closes the real class of silent-loss issue exposed by the absent-skunk symptom.
4. **Add observability and a regression test.** Persist/reload-trace `client_instance_id`, source session ID, target session ID, and attached connection count. Add an exec-handoff test with an attached but non-running TUI plus a second running TUI, then verify both get recovery directives and that `debug reload` from either aliases to the correct target.

This is minimal in behavior: it does not change the actual exec/checkpoint protocol or attempt to serialize sockets. It makes identity handoff explicit and ensures recovery eligibility is based on attachment, not accidental swarm-running state.

## Source reference index

* `crates/jcode-app-core/src/server/reload.rs:140-160` order of intent persistence then shutdown.
* `reload.rs:241-358` candidate selection, directive construction, role assignment, persistence.
* `reload.rs:381-541` signal/checkpoint/wait set, including trigger exclusion at 460-470.
* `crates/jcode-app-core/src/server/reload_recovery.rs:27-73, 187-214, 234-241` record layout, persistence, non-consuming history delivery semantics.
* `crates/jcode-app-core/src/tool/selfdev/reload.rs:18-85, 127-162` reload-context paths and generic/tailored directives.
* `crates/jcode-app-core/src/server/client_state.rs:367-405` recovery directive attached to history.
* `crates/jcode-app-core/src/server/debug_command_exec.rs:46-77` exact debug session resolution and unknown-session failure.
* `crates/jcode-app-core/src/tool/debug_socket.rs:74-100, 105-166` debug tool forwarding of optional request session ID.
* `crates/jcode-app-core/src/server/debug.rs:37-95, 349-427` client-debug mapping is separate from ordinary server debug dispatch.
* `crates/jcode-app-core/src/server/client_lifecycle.rs:562-565` opportunistic global-session fallback update.

## Artifacts inspected

* `~/.jcode/logs/jcode-2026-07-18.log`
* `~/.jcode/reload-traces/reload_1784425095420_8427524887491149517.jsonl`
* `~/.jcode/sessions/session_skunk_1784421354092_7c818fd78fc6f1ae.{json,bak}`
* `~/.jcode/sessions/session_dolphin_1784425094817_a3bab5d9e1f8937a.json`
* `~/.jcode/sessions/session_sauropod_1784425184033_e835b28b189671bb.json`
* `~/.jcode/sessions/session_hedgehog_1784421350156_caa04b8547f40c54.{json,journal.jsonl,evidence.jsonl}`
* `~/.jcode/reload-recovery/` was empty at inspection time. This is expected after hedgehog’s trace-recorded successful delivery and cleanup, and does not erase the durable reload trace.
