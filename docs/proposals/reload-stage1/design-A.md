# Reload Stage 1: checkpoint-and-resume — Design A

Author: Researcher A (independent design track). Grounded directly in the
current tree; every claim about existing behavior cites `file:line`. Where I
could not verify something in the code I have written UNVERIFIED rather than
assert it.

Scope is exactly `docs/proposals/RELOAD_ARCHITECTURE.md`'s Stage 1: stop
treating a liveness flip as proof of saved work, add one control-log event,
emit it from the turn loop, and read it back on resume. No child-process
migration, no Stage-2 broker.

## 0. What exists today (read first)

The reload path is:

`await_reload_signal` (`crates/jcode-app-core/src/server/reload.rs:57`) →
`persist_reload_recovery_intents` (`reload.rs:210`) →
`graceful_shutdown_sessions` → `graceful_shutdown_sessions_with_timeout`
(`reload.rs:330`, `reload.rs:350`) → `replace_process` (`reload.rs:179`) →
`std::process::exit(42)` (`reload.rs:206`).

`graceful_shutdown_sessions_with_timeout` (`reload.rs:350-510`):

1. Snapshots session ids whose `SwarmMember.status == "running"`
   (`reload.rs:359-366`).
2. Splits them into sessions that have a registered `InterruptSignal` in
   `shutdown_signals` vs. not (`reload.rs:368-373`); the unsignalable ones are
   logged and **excluded from the wait entirely** (`reload.rs:375-395`) — they
   never block reload and there is no checkpoint for them today.
3. Fires `InterruptSignal::fire()` on each signalable session's shutdown
   signal (`reload.rs:403-427`). This is exactly `InterruptSignal::fire()` at
   `crates/jcode-agent-runtime/src/lib.rs:50-54`: sets the atomic flag, bumps
   the epoch, and calls `notify_waiters()`.
4. Builds `watched` = signalable sessions minus the triggering session
   (`reload.rs:429-439`) — the triggering session's own signal was already
   fired above, but its `selfdev` tool call resolves synchronously via
   `wait_for_reload_ack` before the process exits, so it is deliberately not
   waited on again (`reload.rs:434-439`; see also
   `crates/jcode-app-core/src/tool/selfdev/reload.rs:412-424`).
5. Loop (`reload.rs:451-509`): while `watched` is non-empty, waits up to the
   2s `RELOAD_GRACEFUL_SHUTDOWN_TIMEOUT` (`reload.rs:14`) on
   `swarm_event_tx` and treats **only these two event shapes** as "this
   session is handled," removing it from `still_running`'s recomputation on
   the next loop iteration by relying on the member's status flipping away
   from `"running"` (`reload.rs:452-465`):
   - `SwarmEventType::StatusChange { .. }` for a watched session id
     (`reload.rs:489`)
   - `SwarmEventType::MemberChange { action: "left" }` for a watched session
     id (`reload.rs:490-491`)

   Crucially, the loop does not act on the *event's* variant at all — it just
   uses the event as a wakeup to re-poll `swarm_members` and check whether
   `status == "running"` still holds (`reload.rs:452-465`). This is the bug
   the ground-truth doc calls out
   (`docs/proposals/RELOAD_ARCHITECTURE.md:42-52`): a session flips to
   `"failed"`, `"ready"`, "left" (client disconnect,
   `crates/jcode-app-core/src/server/client_disconnect_cleanup.rs:203-236`),
   or any other non-`"running"` status and the loop declares it handled,
   with zero connection to whether the in-flight turn's partial output or
   pending tool call was ever durably saved anywhere.

Where `graceful_shutdown` is consumed inside a turn
(`crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs`):

- Before the stream opens: `self.graceful_shutdown.notified()` races the
  `provider.complete_split(..)` future; on fire it returns `Ok(())`
  immediately with **no checkpoint of anything** (turn never started producing
  text) (`turn_streaming_mpsc.rs:247-252`).
- While waiting for the next stream event:
  `self.graceful_shutdown.notified()` races `stream.next()`; on fire it
  `break`s out of the inner event loop, discarding whatever `text_content` /
  `tool_calls` had accumulated so far in this iteration
  (`turn_streaming_mpsc.rs:370-385`, loop starts `turn_streaming_mpsc.rs:363`).
- Inside `TextDelta` handling: `self.is_graceful_shutdown()` is polled after
  each delta; on true it appends a literal
  `"\n\n[generation interrupted - server reloading]"` string to
  `text_content` and `break`s the event loop
  (`turn_streaming_mpsc.rs:547-557`). This is the only place partial
  *assistant text* is captured today — and it is captured only into an
  in-memory local, never into the control log or any durable store before the
  turn tears down.
- After the stream ends, if there are tool calls and
  `self.is_graceful_shutdown()`: every tool result is replaced with
  `"[Skipped - server reloading]"` (is_error) and the loop returns
  (`turn_streaming_mpsc.rs:1200-1217`).
- Inside a single tool's own wait (`turn_streaming_mpsc.rs:1382-1416`): a
  `tokio::select!` races the tool's `JoinHandle` against
  `shutdown_signal.notified()`; on fire, `bash` tools get a 750ms grace
  window to finish (`allow_reload_handoff`, `turn_streaming_mpsc.rs:1383,
  1400-1412`), otherwise the tool is abandoned mid-flight and its message is
  built by `reload_interrupted_tool_result` (`turn_streaming_mpsc.rs:45-76`),
  which is a **special-cased string for `bg`/`swarm` wait-like tools** telling
  the model to rerun the exact same call after reload
  (`turn_streaming_mpsc.rs:55-66`) — this is the existing precedent for "tell
  the model to redo work" and is the closest thing to a resume contract that
  exists today, but it is entirely in the tool-result text, not in any
  durable event.

None of these five checkpoints write anything to `jcode-swarm-core`'s
control log. They only mutate `text_content`/tool-result strings that get
folded into `self.add_message_with_duration(...)` /
`self.session.save()` (`turn_streaming_mpsc.rs:1567-1589`) — i.e. the
*session transcript*, which is real durable storage, but it is not what
`graceful_shutdown_sessions_with_timeout`'s wait loop looks at, and it is not
what the reload wait condition can use as "handled" evidence because the
reload code does not have a session-id-to-control-log-swarm mapping problem —
see §2 for why the control log is still the right home.

Non-streaming path `run_turn` (`crates/jcode-app-core/src/agent/turn_loops.rs:10`)
has **no reference to `graceful_shutdown` or `is_graceful_shutdown()`
anywhere in that file** (verified: `grep -n
"is_graceful_shutdown|graceful_shutdown\.notified"
crates/jcode-app-core/src/agent/turn_loops.rs` returns nothing). Reload
during a `run_turn`-driven turn (used for `--print`/one-shot CLI invocations,
UNVERIFIED which exact call sites use `run_turn` vs
`run_turn_streaming_mpsc`) is out of this design's observed checkpoint
surface; flagged in Risks.

`InterruptSignal` (`crates/jcode-agent-runtime/src/lib.rs:32-117`) has no
payload — `fire()` takes no arguments (`lib.rs:50-54`). It cannot itself carry
checkpoint data; the checkpoint must be written to a separate durable store
before/around the fire reaction, which is exactly what
`RELOAD_ARCHITECTURE.md` specifies (event, not signal payload).

Control log (`crates/jcode-swarm-core/src/control_log.rs`):
`SwarmControlEvent` is a flat `#[serde(tag = "type")]` enum
(`control_log.rs:38-90`); `ArtifactFiled` (`control_log.rs:81-89`) is the
existing "evidence, not liveness" pattern to imitate — it is filed
explicitly by task-completion code
(`crates/jcode-app-core/src/server/comm_graph.rs:466-478`), not derived from
a status transition, and downstream consumers (`comm_await.rs:335`) treat it
as first-class evidence distinct from `TaskStatusChanged`. `apply()`
(`control_log.rs:154-234`) is exhaustive over the enum (no wildcard arm), so
the compiler forces every new variant to get a fold case. Appends go through
`append_control_event(swarm_id, event)`
(`crates/jcode-app-core/src/server/control_log_sync.rs:199-220`), which opens
a per-swarm `ControlLogWriter` and updates a cached in-memory fold
(`control_log_sync.rs:203-213`); it needs a **swarm_id**, not a session_id —
see §2 for the resolution problem this creates.

Recovery/resume machinery
(`crates/jcode-app-core/src/server/reload_recovery.rs`) is a **separate,
session-id-keyed, single-slot JSON file store** (`reload_recovery.rs:61-63`,
`path_for_session`), independent of the control log. It already has the
exact shape Stage 1 needs to extend: `persist_intent` writes a
`ReloadRecoveryRecord{role, status: Pending, directive, reason, ...}`
(`reload_recovery.rs:65-93`); `pending_directive_for_session` reads it
without consuming (`reload_recovery.rs:120-162`), used to attach a
`ReloadRecoverySnapshot` to the client's `History` payload
(`crates/jcode-app-core/src/server/client_state.rs:367-384`);
`mark_delivered_if_matching_continuation` consumes it exactly once, matched
by exact `continuation_message` string equality (`reload_recovery.rs:164-231`),
called from three places: `client_actions.rs:969` (bulk resume of idle
sessions), `client_lifecycle.rs:2712` (a live client accepting a message that
happens to equal the stored continuation), and `server.rs:844` (a third
call site, UNVERIFIED exact context, not read in this pass).
`persist_reload_recovery_intents` (`reload.rs:210-328`) is the write side
called from `await_reload_signal` for every `"running"` swarm member plus the
triggering session, computing a `ReloadRecoveryDirective` from
`ReloadContext::recovery_directive_for_session`
(`crates/jcode-app-core/src/tool/selfdev/reload.rs:151-163`) — this directive
is a **static, session-independent-of-turn-content message** like "Reload
succeeded (v1 → v2). Continue immediately from where you left off." It does
not know anything about what the model was actually saying or doing when the
signal fired. This is the gap Stage 1 closes: today's directive can tell the
model "you were interrupted," but only `TurnStashed` can tell it (and the
resume path) *what specifically was in flight and how to complete it.*

## 1. The wait-condition change

**File**: `crates/jcode-app-core/src/server/reload.rs`,
`graceful_shutdown_sessions_with_timeout`, match arm at `reload.rs:487-493`.

Today:

```rust
match tokio::time::timeout(remaining, event_rx.recv()).await {
    Ok(Ok(event)) => match &event.event {
        SwarmEventType::StatusChange { .. } if watched.contains(&event.session_id) => {}
        SwarmEventType::MemberChange { action }
            if action == "left" && watched.contains(&event.session_id) => {}
        _ => continue,
    },
    ...
```

Change: a session leaves `watched` only when the event proves either (a) the
turn completed normally, or (b) a `TurnStashed` checkpoint was appended for
it. "Completed normally" already has an unambiguous signature in the existing
event stream: `StatusChange { new_status }` where `new_status` is a terminal
non-running state reached *by the normal turn-completion path*
(`"ready"` via `update_member_status_with_report` at
`client_lifecycle.rs:675-686` / `live_turn.rs:134-145`, or `"failed"` via
`update_member_status` at `client_lifecycle.rs:692-702` / `live_turn.rs:153-163`
— note both are reached whether or not a reload was in flight, so this by
itself is not new evidence of *checkpointing*, only of *turn-loop exit*,
which is fine: an unmodified turn that exits and updates status without a
graceful-shutdown signal ever having raced it is legitimately "handled," and
one that *did* see the signal fire but reached a terminal status went through
`is_graceful_shutdown()` branches that either completed the model's last
delta (§0, `TextDelta` interruption still calls
`add_message_with_duration`+`session.save()` before returning
(`turn_streaming_mpsc.rs:1567-1589`)) — the session transcript itself is
already durable in that case. What is *not* covered is the tool-call
mid-flight case at `turn_streaming_mpsc.rs:1400-1416` when the 750ms grace
window is skipped/exceeded and the abandoned tool never gets a
`TurnStashed` — that path still relies on `reload_interrupted_tool_result`'s
inline rerun instruction (§0), which is out of Stage 1's diff surface per the
non-goals below (it is an existing mechanism, not something Stage 1 needs to
touch, since the tool result text itself already tells the model to redo the
exact call).

Concretely:

```rust
match tokio::time::timeout(remaining, event_rx.recv()).await {
    Ok(Ok(event)) => match &event.event {
        SwarmEventType::StatusChange { new_status, .. }
            if watched.contains(&event.session_id)
                && matches!(new_status.as_str(), "ready" | "failed" | "completed" | "crashed" | "stopped") => {}
        SwarmEventType::MemberChange { action }
            if action == "left" && watched.contains(&event.session_id) => {}
        _ => continue,
    },
```

Wait — this is *not* actually the fix, and I want to flag why plainly rather
than paper over it: filtering `StatusChange` by terminal `new_status` values
does not distinguish "turn finished and checkpointed" from "turn finished
after silently dropping partial work," because *every* graceful-shutdown exit
path in `turn_streaming_mpsc.rs` reaches one of exactly these same terminal
statuses via the normal `update_member_status[_with_report]` call at the end
of `process_message_streaming_mpsc`/`spawn_tracked_live_turn` regardless of
whether a checkpoint happened. The status transition is orthogonal to
checkpoint evidence — this is the same class of bug (F2, per the control_log
docstring at `control_log.rs:1-20`) that motivated `ArtifactFiled` in the
first place.

**Corrected design**: stop keying the wait condition on `SwarmEventType` at
all for the reload-interrupted case, and instead make the loop's "still
running" recomputation ask two questions per watched session, not one:

1. Is `swarm_members[id].status != "running"` (existing check,
   `reload.rs:452-465`)? If false, still running, keep waiting — unchanged.
2. If true (status left `"running"`), was this session's exit *preceded by*
   the shutdown signal actually firing while it still had unflushed partial
   work? We do not need to inspect that in the reload wait loop at all — we
   need the loop to keep waiting for `TurnStashed` (or session-exit) as two
   independent satisfying conditions, and drop the assumption that "status
   changed" alone is sufficient. Restated as the actual patch:

```rust
match tokio::time::timeout(remaining, event_rx.recv()).await {
    Ok(Ok(event)) => match &event.event {
        // Proof the turn loop reached its normal, non-shutdown-forced exit:
        // MemberChange{"left"} (client disconnected / session removed,
        // client_disconnect_cleanup.rs:222-224) still means "nothing to wait
        // for," since a departed session cannot be resumed by --resume
        // anyway (its recovery record targets a session id nothing will
        // reconnect as). Kept as-is.
        SwarmEventType::MemberChange { action }
            if action == "left" && watched.contains(&event.session_id) => {}
        // NEW: explicit checkpoint evidence, independent of member status.
        // Emitted by the turn loop (see §3) into the session's swarm's
        // control log; the reload wait loop treats *this*, not a status
        // flip, as proof of saved work.
        SwarmEventType::TurnCheckpointed { .. }
            if watched.contains(&event.session_id) => {}
        _ => continue,
    },
```

This requires a **new in-memory `SwarmEventType::TurnCheckpointed { session_id
carried on the envelope already }`** broadcast alongside the durable
`TurnStashed` control-log write (§2), for the same reason `StatusChange` is
both recorded to `event_history`/control-log-adjacent bookkeeping *and*
broadcast on `swarm_event_tx` — the reload wait loop only has a
`broadcast::Receiver<SwarmEvent>`, it does not scan the control log file
directly (that machinery lives in `control_log_sync.rs` and is swarm-id
keyed, async-file-IO based, and not currently wired to `reload.rs`'s
`swarm_event_tx` consumer). Concretely, alongside the `TurnStashed`
control-log append (§2/§3), emit one `record_swarm_event(...,
SwarmEventType::TurnCheckpointed { .. })`
(the same helper `StatusChange` events already go through, e.g.
`swarm.rs:1568-1580`) so the reload wait loop's existing
`event_rx.recv()`-based mechanism picks it up with no new subscription
plumbing.

Also **do not remove** the `SwarmEventType::StatusChange` arm outright — a
session that exits *without ever having its shutdown signal fire* (i.e. it
finished its own turn coincidentally during the graceful-shutdown window, or
it was never actually generating despite being snapshotted as `"running"`
at `reload.rs:359-366`, a real race since that snapshot and the `fire()` calls
are not atomic with the turn loop's own state) has nothing to checkpoint and
should not block reload. Keep a **narrower** `StatusChange` arm gated on "this
session's shutdown signal was never actually observed as fired," which is not
directly knowable from `SwarmEvent` today. Simplest correct fix without new
per-signal bookkeeping: track *epoch at fire time* (already available via
`InterruptSignal::epoch()`, `lib.rs:67-69`) and compare it to whatever
epoch the turn loop reports it observed in its `TurnCheckpointed`/terminal
event — UNVERIFIED whether plumbing that through is worth it for Stage 1; the
pragmatic compromise below is what I recommend actually shipping.

**Recommended, minimal patch** (avoids inventing an epoch-comparison
protocol): keep the existing `StatusChange`/`MemberChange{"left"}` arms
exactly as they are (they already correctly retire the *few-hundred-ms* race
window between "session snapshotted as running" and "session's own turn
loop finished before observing the fired signal" — that is a real, benign
case, not the bug), and **add** the `TurnCheckpointed` arm as a third
satisfying condition. The bug the ground-truth doc describes — "timed out"
and "left with nothing saved" being indistinguishable — is not actually fixed
by removing the existing arms (a session that exits with nothing to save
*should* unblock reload immediately, that is correct behavior); it is fixed
by *also* being able to see, from server-side logs/telemetry, whether a
`TurnCheckpointed`/`TurnStashed` occurred for a given session before it
exited. That observability is the actual deliverable: log a line inside the
loop (or in `persist_reload_recovery_intents`, which already runs first and
has the candidate list) distinguishing "session exited with a stashed turn
recovered" vs. "session exited with none," so the previously-conflated cases
become distinguishable in `crate::logging`/`reload_trace` — both of which
already exist as the tracing mechanism for this exact function
(`reload_trace::record_value`, used throughout `reload.rs`, e.g.
`reload.rs:414-421`, `reload.rs:471-475`(not literal, paraphrase — actual
line `reload.rs:376-388` for the unsignalable-sessions warn)).

Restated as the actual code diff (this is the version to implement):

```rust
// reload.rs, inside graceful_shutdown_sessions_with_timeout's loop, replacing
// the match at reload.rs:488-493:
match tokio::time::timeout(remaining, event_rx.recv()).await {
    Ok(Ok(event)) => match &event.event {
        SwarmEventType::StatusChange { .. } if watched.contains(&event.session_id) => {}
        SwarmEventType::MemberChange { action }
            if action == "left" && watched.contains(&event.session_id) => {}
        _ => continue,
    },
```

stays structurally the same (still_running recomputation via
`swarm_members[id].status` at `reload.rs:452-465` is what actually retires a
session from the wait set — the match arms are just wakeup filters, not the
source of truth), **plus** a new tracing line right after the
`still_running.is_empty()` break (`reload.rs:466-469`) and right after the
timeout-warn (`reload.rs:478-485`) that records, per session that left
`watched`, whether `ReloadContext::peek_for_session`/the new
`peek_turn_stashed_for_session` (§4) found a stashed turn — making "left with
a checkpoint" vs. "left with nothing" a queryable, testable fact instead of
an indistinguishable liveness flip. This satisfies the letter of the
ground-truth doc's complaint (make the two cases distinguishable) with a much
narrower diff than restructuring the match arms, and is honest about the fact
that filtering on `SwarmEventType` variants alone cannot encode "was this
particular liveness flip *preceded by* a successful checkpoint" without
either (a) a new event carrying that fact explicitly (`TurnCheckpointed`,
which I ultimately recommend adding — see below) or (b) cross-referencing a
second store.

**Final recommendation, stated once, unambiguously**: implement the
`TurnCheckpointed` broadcast event (new `SwarmEventType` variant) and add it
as a third match arm alongside the existing two. It is the smallest change
that actually makes "handled" mean what the ground-truth doc says it should
mean, it reuses `record_swarm_event`'s existing plumbing
(`swarm.rs:1568-1580` is the call shape to imitate), and it does not require
touching `SwarmControlEvent`'s fold semantics for something that is purely a
transient reload-coordination signal, not swarm control state. Keep
`SwarmControlEvent::TurnStashed` (§2) as the **durable** record read back on
resume, and `SwarmEventType::TurnCheckpointed` as the **transient**
broadcast wakeup for the reload wait loop specifically — mirroring exactly
how `ArtifactFiled` (durable, control log) and the `comm_await.rs` wake
predicate (`comm_await.rs:335`, using `swarm_event_tx` as a nudge only, per
the docstring at `control_log_sync.rs:301-311`) are already split.

## 2. `SwarmControlEvent::TurnStashed`

**File**: `crates/jcode-swarm-core/src/control_log.rs`, new variant added to
the enum at `control_log.rs:38-90`, alongside `ArtifactFiled`:

```rust
/// Stage-1 reload checkpoint: a turn was interrupted by a graceful-shutdown
/// signal (server reload) with partial assistant output and/or a
/// resumable request still in flight. Unlike `TaskStatusChanged`, this is
/// direct evidence a turn was checkpointed, not a liveness/status flip
/// (same "evidence, not liveness" contract as `ArtifactFiled`).
TurnStashed {
    session_id: String,
    /// Partial assistant text accumulated before the interrupt, if any.
    /// Already persisted into the session transcript by the turn loop
    /// (turn_streaming_mpsc.rs:1567-1589); carried here too so the reload
    /// wait loop and resume path do not need to open the session store to
    /// know a checkpoint happened and roughly what it contained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    partial_content: Option<String>,
    /// The request to re-issue on resume: the user-facing continuation
    /// message the model should receive (mirrors
    /// `ReloadRecoveryDirective::continuation_message`,
    /// jcode-selfdev-types). Kept as a plain string, not a structured
    /// request, so resume can lean on provider prompt-prefix caching by
    /// simply re-sending the existing session transcript plus this message
    /// (see §4) rather than reconstructing an API request shape here.
    resume_request: String,
},
```

Fold (`SwarmControlState::apply`, `control_log.rs:154-234`): add a matching
arm. `TurnStashed` needs a home in `SwarmControlState`. Reusing
`TaskControlState.last_artifact`'s pattern (`control_log.rs:141-147`) is
wrong — a stashed turn is not associated with a `task_id`, it is associated
with a `session_id`, and `MemberControlState` (`control_log.rs:130-134`) has
no field for it today. Add:

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemberControlState {
    pub role: String,
    pub status: String,
    pub friendly_name: Option<String>,
    /// Most recent stashed-turn checkpoint for this member, if any is
    /// still outstanding. `None` once the member's next `MemberStatusChanged`
    /// or a fresh `TurnStashed` for the same session_id supersedes it —
    /// see the apply() arm below for exactly when it clears.
    pub stashed_turn: Option<StashedTurn>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StashedTurn {
    pub partial_content: Option<String>,
    pub resume_request: String,
}
```

```rust
SwarmControlEvent::TurnStashed {
    session_id,
    partial_content,
    resume_request,
} => {
    self.members.entry(session_id.clone()).or_default().stashed_turn =
        Some(StashedTurn {
            partial_content: partial_content.clone(),
            resume_request: resume_request.clone(),
        });
}
```

Clearing: do **not** add an implicit auto-clear inside `apply()` for other
variants (e.g. clearing `stashed_turn` on `MemberStatusChanged`) — that would
make the fold's behavior for one variant depend on interpreting another,
which the module's own docstring warns against ("Payload discipline: events
carry the transition, not the world," `control_log.rs:33-37`). Instead,
resume consumption is what clears it, exactly like `ArtifactFiled`'s
`last_artifact` is never cleared by the fold either (it is left as permanent
evidence; consumers that care about "still pending" already layer a second
store — `reload_recovery.rs`'s own `Pending`/`Delivered` status is that
layer, see §4). This keeps `apply()` a pure, order-independent fold
(replay determinism, `control_log.rs:19-20`) and pushes "is this still
unconsumed" bookkeeping to the same place it already lives:
`reload_recovery.rs`'s per-session `ReloadRecoveryStatus`.

**Where `TurnStashed` gets appended — swarm_id resolution problem**:
`append_control_event(swarm_id, event)`
(`control_log_sync.rs:199-220`) is keyed by **swarm_id**, not session_id.
Every existing call site derives `swarm_id` from `SwarmMember.swarm_id`
(`state.rs:200`, `Option<String>`) which is only populated when
`swarm_enabled` (`client_session.rs:311-315`, gated by
`crate::config::config().features.swarm`,
`client_lifecycle.rs:426`). **A session with swarm disabled has no
swarm_id, and therefore no control log to append `TurnStashed` to.** This is
a real gap: the ground-truth doc's step 2
(`RELOAD_ARCHITECTURE.md:53-56`) says "add a `SwarmControlEvent::TurnStashed`
variant" without addressing that the control log is fundamentally a
per-swarm structure and Stage 1 needs per-session durability regardless of
swarm membership (a lone, non-swarm interactive session reloading is
probably the *most common* case Stage 1 needs to fix, not the least).

Two options, stated plainly rather than picked silently:

- **(a)** Only append `TurnStashed` when `swarm_id.is_some()`; for
  non-swarm sessions, skip the control-log write and rely solely on the
  existing `reload_recovery.rs` mechanism (§4) to carry the resume request,
  same as today's `ReloadRecoveryDirective`. This keeps the control-log
  event genuinely swarm-scoped (consistent with everything else in
  `control_log.rs`) but means `TurnStashed` provides value only for swarm
  members, which covers the coordinator/worker reload case explicitly called
  out as high-value in the ground-truth doc's Stage-2 motivation section
  (`RELOAD_ARCHITECTURE.md:9-13` — "running child processes... swarm
  progress") but does not cover a plain interactive `jcode` session with
  swarm off.
- **(b)** Extend `reload_recovery.rs` (the session-id-keyed store that
  already exists for exactly this purpose) with the `TurnStashed` payload
  instead of, or in addition to, the swarm control log. This is a smaller
  diff, reuses a store already proven to survive `execve` (it is a plain
  JSON file under `crate::storage::jcode_dir()`, `reload_recovery.rs:57-59`,
  not in-memory), and does not require solving the swarm_id-for-non-swarm-
  session problem at all.

**I recommend (b) as the actual implementation, with the control-log event
as the SESSION'S DURABLE RECORD ONLY WHEN swarm-enabled, and
`reload_recovery.rs` as the universal fallback that always works.**
Concretely: `TurnStashed`'s fields land in a new field on
`ReloadRecoveryRecord` (`reload_recovery.rs:31-42`) —

```rust
pub(super) struct ReloadRecoveryRecord {
    pub reload_id: String,
    pub session_id: String,
    pub role: ReloadRecoveryRole,
    pub status: ReloadRecoveryStatus,
    pub directive: ReloadRecoveryDirective,
    pub reason: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<String>,
    /// Stage-1: partial output/resume-request captured by the turn loop's
    /// InterruptSignal reaction, if the interrupted session had one. `None`
    /// for sessions recovered via the pre-existing ReloadContext path only
    /// (no turn was actually mid-flight when the signal fired).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stashed_turn: Option<StashedTurnRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct StashedTurnRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_content: Option<String>,
    pub resume_request: String,
}
```

and the control-log `TurnStashed` variant (still worth adding per the
ground-truth doc's explicit instruction) is appended *additionally*, gated
on `swarm_id.is_some()`, purely so swarm-aware consumers (coordinators
running `swarm await_members`, `comm_await.rs`-style wake predicates) can
observe "a peer stashed a turn" as first-class evidence rather than needing
to poll `reload_recovery.rs`'s per-session files, which have no swarm-scoped
enumeration today (`path_for_session` is a direct filename-by-session-id
lookup, `reload_recovery.rs:61-63`, not a directory scan). This is the
`ArtifactFiled`-shaped event the ground-truth doc asked for, used exactly
where it adds value (swarm coordination), without forcing every reload path
through the swarm subsystem.

## 3. Where the turn loop emits the checkpoint

**File**: `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs`. Per §0
there are three `graceful_shutdown` reaction points that discard state
without ever writing anything beyond the in-process `text_content` local
and (in one case) the session transcript:

1. `turn_streaming_mpsc.rs:247-252` — signal fires before the stream opens.
   Nothing has been sent to the provider yet; the "resume request" is simply
   the *next* pending user/system message that was about to be sent this
   turn — i.e. nothing was lost, the turn just never started. **No
   `TurnStashed` needed here**: the caller that invoked this turn (an
   unconsumed client message, or `spawn_tracked_live_turn`'s `message`
   argument) is itself still sitting wherever it came from (client socket
   send, or `reload_recovery.rs`'s own directive for the *previous* reload,
   if this is a resumed turn that got re-interrupted). This path returns
   `Ok(())` with no side effect, which is already correct — the caller's
   existing error/retry handling (`client_lifecycle.rs:690-712`,
   `live_turn.rs:148-169`) already surfaces a `failed` status and the
   client-visible message is whatever was already queued. No new work.

2. `turn_streaming_mpsc.rs:370-385` — signal fires while waiting for a
   stream event, mid-response. This is the one true "partial content in
   flight" case: `text_content` may hold a partial assistant response, and
   `tool_calls` may hold partially-parsed tool invocations that never got
   dispatched. **This is where `TurnStashed` must be emitted.**

3. `turn_streaming_mpsc.rs:1200-1217` — signal fires after the stream ended
   with tool calls pending dispatch. `text_content` at this point is the
   *complete* assistant message (already added via
   `add_message_ext`/`session.save()`, see the flow at
   `turn_streaming_mpsc.rs:1082-1122`, UNVERIFIED exact save call site in
   this excerpt but consistent with the pattern at
   `turn_streaming_mpsc.rs:1567-1576` for the sibling tool-interrupt path);
   what's actually lost here is the **tool calls that were never run**. This
   is covered by the existing `"[Skipped - server reloading]"` tool-result
   mechanism (`turn_streaming_mpsc.rs:1205-1214`) which already tells the
   model, in-transcript, that these tools did not run — which the model will
   see on its very next turn (the transcript is durable,
   `self.session.save()` at `turn_streaming_mpsc.rs:1215`) and can decide to
   re-issue. **This already satisfies "resume request" for this case without
   a new event** — a `TurnStashed` here would be redundant with what the
   transcript already durably contains. I recommend *not* emitting one here,
   to keep the diff narrow per the constraint, and treating this as evidence
   the wait-condition's existing `StatusChange`-based "handled" (the turn
   loop reaches `break` then the normal end-of-turn status update fires) is
   correct behavior for this case.

**Concrete change at case 2** (`turn_streaming_mpsc.rs:370-385`):

```rust
_ = self.graceful_shutdown.notified() => {
    log_agent_provider_stream_lifecycle(
        logging::LogLevel::Warn,
        self,
        "stream_cancelled",
        api_start,
        vec![
            ("mode", "mpsc".to_string()),
            ("reason", "graceful_shutdown".to_string()),
        ],
    );
    logging::info(
        "Graceful shutdown/cancel while waiting for API stream event - stopping stream",
    );
    self.stash_interrupted_turn(&text_content, &send_messages_owned_or_ref);
    break;
}
```

New method on `Agent` (new file or appended to `interrupts.rs`, which already
owns `graceful_shutdown_signal()`/`request_graceful_shutdown()`/
`is_graceful_shutdown()`, `interrupts.rs:166-176`):

```rust
/// Stage-1 reload checkpoint: called from the turn loop's graceful-shutdown
/// reaction, BEFORE the in-flight request is torn down (i.e. while
/// `text_content`/pending tool-call state is still live in the caller's
/// locals). Persists a resumable checkpoint so a `--resume`d server does
/// not need the model to reconstruct what it was doing from scratch.
///
/// This method owns the checkpoint; reload.rs only fires the signal that
/// causes this to run (RELOAD_ARCHITECTURE.md's explicit ownership split).
pub(super) fn stash_interrupted_turn(&self, partial_content: &str) {
    // The resume request IS the original next-turn continuation directive
    // (recovery_directive_for_session, already computed post-reload today)
    // PLUS an instruction to pick up the partial response. Building the
    // exact resend string here, rather than in reload_recovery.rs, keeps
    // "what does resuming a stashed turn say to the model" co-located with
    // the turn-loop code that knows what was actually interrupted.
    let resume_request = if partial_content.trim().is_empty() {
        "Your previous response was interrupted by a server reload before \
         any output was produced. Continue exactly where you left off; \
         reissue the same request if needed.".to_string()
    } else {
        format!(
            "Your previous response was interrupted by a server reload \
             partway through. The partial response you had produced was: \
             {:?}\n\nContinue exactly from where that partial response left \
             off. Do not repeat the text above; complete the thought and \
             continue the turn.",
            partial_content.trim()
        )
    };

    if let Err(err) = crate::server::reload_recovery::stash_turn_for_session(
        &self.session.id,
        partial_content,
        &resume_request,
    ) {
        logging::warn(&format!(
            "Failed to stash interrupted turn for session {}: {}",
            self.session.id, err
        ));
    }
}
```

(`crate::server::reload_recovery` is currently `pub(super)` — every function
in it is `pub(super) fn`, e.g. `reload_recovery.rs:65,95,120,164` — so a new
`stash_turn_for_session` following the same visibility and the agent module
calling into `crate::server::...` needs that module's visibility widened
enough for `crate::agent` to reach it, or the function relocated. UNVERIFIED
exact minimal visibility fix; likely `pub(crate)` on
`stash_turn_for_session` specifically rather than opening the whole module,
consistent with how narrowly-scoped the rest of that file already is.)

`stash_turn_for_session` in `reload_recovery.rs`, sibling to
`persist_intent` (`reload_recovery.rs:65-93`):

```rust
pub(crate) fn stash_turn_for_session(
    session_id: &str,
    partial_content: &str,
    resume_request: &str,
) -> Result<()> {
    let path = path_for_session(session_id)?;
    let mut record: ReloadRecoveryRecord = if path.exists() {
        crate::storage::read_json(&path)?
    } else {
        // No pre-existing directive (e.g. this session was not one of the
        // reload's precomputed candidates, or persist_reload_recovery_intents
        // has not run yet due to ordering — see the race note below). Create
        // a minimal record so the stash is not lost.
        ReloadRecoveryRecord {
            reload_id: "turn-stashed-only".to_string(),
            session_id: session_id.to_string(),
            role: ReloadRecoveryRole::InterruptedPeer,
            status: ReloadRecoveryStatus::Pending,
            directive: ReloadRecoveryDirective {
                reconnect_notice: None,
                continuation_message: resume_request.to_string(),
            },
            reason: "turn interrupted by graceful shutdown signal".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            delivered_at: None,
        }
    };
    record.status = ReloadRecoveryStatus::Pending;
    record.stashed_turn = Some(StashedTurnRecord {
        partial_content: if partial_content.trim().is_empty() {
            None
        } else {
            Some(partial_content.to_string())
        },
        resume_request: resume_request.to_string(),
    });
    // The stashed resume request supersedes whatever static directive
    // persist_reload_recovery_intents may write later for this session,
    // since it is turn-content-aware. See the ordering note below for why
    // "later" needs an explicit tie-break.
    record.directive.continuation_message = resume_request.to_string();
    crate::storage::write_json(&path, &record)?;
    crate::logging::info(&format!(
        "reload recovery store: stashed interrupted turn session={} partial_chars={} reload_id={}",
        session_id, partial_content.len(), record.reload_id,
    ));
    Ok(())
}
```

**Ordering hazard, called out explicitly**: `persist_reload_recovery_intents`
runs in `await_reload_signal` *before* `graceful_shutdown_sessions` fires the
signals (`reload.rs:117-137`), so it always writes its static directive
first. `stash_turn_for_session` runs later, inside the turn loop's async
reaction to the fired signal, and — because it reads-modifies-writes the same
file `persist_intent` just wrote — will correctly read that pre-existing
record and overwrite its `continuation_message` with the richer,
turn-content-aware one. This is intentional: it is a last-write-wins merge
where the turn-loop's write is guaranteed to be causally later (it can only
run after the signal that `persist_reload_recovery_intents` blocks on having
already sent). No lock is needed for this because both writers run on the
same process's tokio runtime and the write is a single non-interleaved
`storage::write_json` call, but there IS a genuine TOCTOU race between
`stash_turn_for_session`'s read and its write if two different signals could
fire concurrently for the same session — UNVERIFIED whether that can happen
(a session only has one `InterruptSignal` registered in `shutdown_signals`,
and only one turn loop instance owns it at a time per the `Mutex<Agent>`
session-lock model, so I believe it cannot, but I did not trace the full
locking discipline to be certain).

## 4. Resume read-back path

**File**: `crates/jcode-app-core/src/server/client_state.rs`,
`history_reload_recovery_snapshot` (`client_state.rs:367-390`) already reads
`pending_directive_for_session` (`client_state.rs:371`) and attaches it to
the `History` payload without consuming it — this is the primary
`--resume`/reconnect path a client hits. **No new read-back call site is
needed here**: because `stash_turn_for_session` (§3) already overwrites
`record.directive.continuation_message` in place, the existing
`pending_directive_for_session` read automatically returns the richer,
turn-aware resume request once §3 ships, with zero changes to
`client_state.rs`.

The bulk-resume path, `client_actions.rs:958-984` (used for
`resume_all_sessions`-style flows on server startup — UNVERIFIED exact
trigger, but the function name and surrounding
`live_session_owes_continuation` check at `client_actions.rs:952` strongly
suggest this is the auto-continue-on-reconnect path), similarly already
reads `pending_directive_for_session` (`client_actions.rs:958`) and passes
`directive.continuation_message` as the `system_reminder` into
`spawn_tracked_live_turn` (`client_actions.rs:980-990`) — this is the actual
"re-issue the request" mechanism the ground-truth doc's step 4 asks for
(`RELOAD_ARCHITECTURE.md:61-63`). It re-issues by injecting the resume
request as a system reminder on the *next* turn against the *same session
transcript*, which is exactly "lean on provider prompt-prefix caching for
the resend rather than adding a custom deduplication layer" — the provider
sees the same message history up to the interruption point plus one new
system-reminder-prefixed turn, so the provider's own prompt cache (already
relied upon elsewhere in this codebase per
`turn_streaming_mpsc.rs:212-220`'s `cache_signature_messages`/
`kv_cache_request_event`) does the heavy lifting with no new logic required.

**What Stage 1 DOES need to add here**: `mark_delivered_if_matching_continuation`
(`reload_recovery.rs:164-231`) matches on exact string equality of
`continuation_message` (`reload_recovery.rs:188`). Since §3 now writes a
turn-content-specific string (containing the actual partial response text),
this equality check still works correctly as long as the same string that
was read via `pending_directive_for_session`/the `directive` field is what
gets passed back into `mark_delivered_if_matching_continuation` — which is
already the pattern at `client_actions.rs:958-978` (`reminder` is read from
the directive, then the identical `&reminder` is passed to
`mark_delivered_if_matching_continuation` at `client_actions.rs:970`). **No
new field or comparison logic needed** — the existing exact-match consumption
mechanism transparently now also consumes stashed-turn resume requests,
because they live in the same `directive.continuation_message` field.

One addition IS needed: exposing `stashed_turn.partial_content` (the
literal partial text, separate from the resume instruction wrapped around
it) to anything that wants to show the user/model what was actually lost,
beyond what's already embedded in the `resume_request` string. For Stage 1's
narrow scope this is optional — the resume request string already contains
the partial content inline (§3's `format!`) — so I recommend **not** adding a
separate read path for `stashed_turn` as a distinct field consumers query;
let it ride inside `continuation_message` and keep `reload_recovery.rs`'s
public read surface exactly as it is today (`pending_directive_for_session`,
`mark_delivered_if_matching_continuation`) plus the one new write function
(`stash_turn_for_session`). This is the narrowest correct resume path.

## 5. Non-goals (explicit)

- **No live child-process migration.** `bash`/tool subprocesses in flight at
  reload time are still handled exactly as today
  (`turn_streaming_mpsc.rs:1382-1416`'s 750ms grace window, then abandonment
  + `reload_interrupted_tool_result`'s rerun-instruction text). Stage 1 does
  not SIGTERM or reparent anything new. `persist_reload_recovery_intents`
  already recovers session/task role (`reload.rs:210-328`); it is not
  extended to enumerate live OS child processes.
- **No Stage-2 broker.** No new long-lived process, no UDS protocol change,
  no LLM-stream ownership transfer. The provider stream itself is still lost
  on `execve`; Stage 1 only makes what was *said so far* (and what needs to
  be said next) durable and resumable, per
  `RELOAD_ARCHITECTURE.md:7-25`'s hard-constraint framing.
- **No custom dedup layer for the resend.** §4 explicitly relies on the
  provider's own prompt-prefix caching (already used elsewhere,
  `turn_streaming_mpsc.rs:212-220`) rather than inventing a
  request-id/idempotency-key scheme.
- **No change to the `run_turn` (non-streaming) path.** Per §0, it has no
  `graceful_shutdown` awareness at all today; Stage 1 does not add it. Any
  reload during a `run_turn`-driven turn is unaffected by this design and
  behaves exactly as it does today (UNVERIFIED exactly what that is, since
  the file has no shutdown-signal handling to trace).
- **No new supervision surface.** `shutdown_signals`
  (`Arc<RwLock<HashMap<String, InterruptSignal>>>`) and `swarm_members`
  remain the only two maps `reload.rs` reads. No new process registry, no
  new health-check loop.

## 6. Implementation checklist

1. `jcode-swarm-core/src/control_log.rs`: add `SwarmControlEvent::TurnStashed`
   variant + `StashedTurn`/`MemberControlState.stashed_turn` fold support +
   unit test asserting fold determinism (replay twice, same result) and that
   an unrelated `MemberStatusChanged` does NOT clear `stashed_turn` (locks in
   the "events carry the transition, not the world" contract from §2).
2. `jcode-app-core/src/server/reload_recovery.rs`: add `StashedTurnRecord`,
   `ReloadRecoveryRecord.stashed_turn` field (with `#[serde(default)]` for
   backward-compat with existing on-disk records that predate this field),
   and `stash_turn_for_session(...)`. Unit tests: (a) stash onto an existing
   pending record overwrites `continuation_message` and preserves `role`;
   (b) stash with no pre-existing record creates a minimal one; (c) a
   subsequent `mark_delivered_if_matching_continuation` with the stashed
   string consumes it exactly like today's directive-only case.
3. `jcode-app-core/src/agent/interrupts.rs` (or a new
   `agent/turn_checkpoint.rs`): add `Agent::stash_interrupted_turn`. Widen
   `reload_recovery`'s visibility just enough for `crate::agent` to call
   `stash_turn_for_session` (`pub(crate)` on that one function, per the
   UNVERIFIED note in §3 — confirm the minimal visibility change compiles
   without opening the whole module).
4. `jcode-app-core/src/agent/turn_streaming_mpsc.rs`: call
   `self.stash_interrupted_turn(&text_content)` at the mid-stream
   `graceful_shutdown.notified()` branch (`turn_streaming_mpsc.rs:370-385`),
   before the `break`. Do NOT add a call at the pre-stream-open branch
   (`turn_streaming_mpsc.rs:247-252`) or the post-stream tool-skip branch
   (`turn_streaming_mpsc.rs:1200-1217`) per §3's analysis.
5. New `SwarmEventType::TurnCheckpointed { session_id-carrying envelope as
   existing variants do }` in `state.rs` (near `StatusChange`,
   `state.rs:344-348`), broadcast via the existing `record_swarm_event`
   helper immediately after `stash_turn_for_session` succeeds — call site is
   inside `stash_interrupted_turn`, which needs a `swarm_event_tx` handle;
   confirm whether `Agent` already has one reachable (UNVERIFIED — not
   traced in this pass; if not, thread it through the same way
   `event_tx: mpsc::UnboundedSender<ServerEvent>` already reaches
   `run_turn_streaming_mpsc`, or fire it from the server-side caller instead
   of from inside `Agent` — see Risks).
6. `jcode-app-core/src/server/reload.rs`,
   `graceful_shutdown_sessions_with_timeout`: add the
   `SwarmEventType::TurnCheckpointed { .. } if watched.contains(...)` match
   arm at `reload.rs:487-493`. Add a `reload_trace::record_value` call
   distinguishing "session left watched set via checkpoint" vs. "via
   status/left" for observability (§1's actual deliverable).
7. Test: extend `reload_tests.rs`
   (`crates/jcode-app-core/src/server/reload_tests.rs`, referenced at
   `reload.rs:512-514`) with a case that fires a session's shutdown signal,
   asserts the wait loop blocks until a `TurnCheckpointed` event is
   broadcast (not just any `StatusChange`), and that a bare `StatusChange`
   to a terminal status without a preceding checkpoint still correctly
   unblocks the wait (per §1's decision to keep both arms) — this is the
   regression test that would have caught the original bug and would catch
   a future regression that removes the checkpoint distinction.
8. Test: round-trip `stash_turn_for_session` →
   `pending_directive_for_session` → `mark_delivered_if_matching_continuation`
   in `reload_recovery.rs`'s existing test module, mirroring
   `pending_directive_does_not_consume_intent`
   (`reload_recovery.rs:340-362`) and
   `mark_delivered_is_idempotent_and_matches_exact_continuation`
   (`reload_recovery.rs:364-410`).
9. End-to-end (if the existing reload e2e harness supports mid-turn
   injection — UNVERIFIED, not located in this pass): interrupt a real
   streaming turn mid-`TextDelta`, trigger reload, assert the replacement
   server's `History` payload for that session contains a
   `continuation_message` mentioning the partial text.

## 7. Risks

- **`Agent` may not have a `swarm_event_tx` handle available inside
  `turn_streaming_mpsc.rs`'s checkpoint call site.** `run_turn_streaming_mpsc`
  takes `event_tx: mpsc::UnboundedSender<ServerEvent>` (client-facing), not
  `broadcast::Sender<SwarmEvent>` (swarm-internal) — these are two different
  channel types serving two different audiences (`state.rs` types vs
  `protocol.rs` types, UNVERIFIED exact module for `ServerEvent`). If `Agent`
  genuinely cannot reach the swarm broadcast channel, `TurnCheckpointed`
  either needs to be fired from the *server-side* caller after the turn
  future returns (weakening the "before it tears down the in-flight request"
  timing the ground-truth doc asks for, since the server-side caller only
  learns the turn ended, not that it specifically hit the graceful-shutdown
  branch with a stash) or `stash_turn_for_session`'s success needs to be
  observable some other way (e.g. the reload wait loop polling
  `reload_recovery.rs` directly instead of via a broadcast event, trading the
  "no new subscription plumbing" benefit claimed in §1 for a small polling
  loop). This is the single biggest unresolved wiring question in this
  design and should be resolved by actually reading `Agent`'s full field
  list and `run_turn_streaming_mpsc`'s call site in `client_lifecycle.rs`
  before implementation starts.
- **The reload wait loop's 2-second timeout may be too short for
  `stash_turn_for_session`'s disk write plus the broadcast round-trip**,
  especially if many sessions are checkpointing concurrently at reload time
  (each is a separate `storage::write_json` — synchronous file IO, per
  `reload_recovery.rs:84`). Not measured; the ground-truth doc explicitly
  says "measure token loss on reload before and after" — this design does
  not include that measurement harness, only the mechanism.
- **`persist_reload_recovery_intents` and `stash_turn_for_session` both
  read-modify-write the same per-session JSON file with no advisory lock**
  (`crate::storage::write_json` — UNVERIFIED whether it does any locking
  internally; not traced). The ordering argument in §3 relies on
  single-process, single-tokio-runtime causal ordering, which holds today
  but is fragile to future changes (e.g. if reload logic ever moves
  cross-process).
- **Backward compatibility of the new `ReloadRecoveryRecord.stashed_turn`
  field** with any already-serialized `reload-recovery/*.json` files from a
  previous binary version left on disk across the exact reload this design
  ships in. `#[serde(default)]` handles missing-field deserialization
  correctly (same pattern already used for `delivered_at`,
  `reload_recovery.rs:40-41`), so this should be safe, but is called out
  since it's the literal first reload after this change ships.
- **`SwarmControlEvent::TurnStashed`'s value is limited to swarm-enabled
  sessions** (§2's option (a)/(b) split) — if a future reviewer expects the
  control-log event to be the universal mechanism per a literal reading of
  the ground-truth doc's step 2, this design's choice to make
  `reload_recovery.rs` the universal store and the control log an
  additional swarm-visible echo needs explicit sign-off, not silent
  adoption.

## 8. What I did not check

- Whether `Agent` has any field or method that already exposes a
  `broadcast::Sender<SwarmEvent>` or equivalent — I searched for
  `swarm_event_tx` usage across `server/*.rs` but did not grep `agent.rs`'s
  full field list or trace every constructor call site.
- The exact call sites that choose `run_turn` vs `run_turn_streaming_mpsc` —
  I confirmed `run_turn_streaming_mpsc` is what `client_lifecycle.rs`'s
  `process_message_streaming_mpsc` and `live_turn.rs` use, but did not find
  where (or whether, in the currently-shipped binary) `run_turn` is invoked
  in a live server request path versus only CLI one-shot/print modes.
- `reload_trace::record_value`'s consumer — I cited it as the existing
  tracing mechanism but did not open `reload_trace.rs` to confirm what reads
  these values back (a debug endpoint? a test assertion helper? both?).
- `server.rs:844`'s exact context for its
  `mark_delivered_if_matching_continuation` call — a third call site I found
  by grep but did not read in full; it may have implications for how many
  places need to agree on the stashed-turn string.
- Whether `storage::write_json` provides any file-level locking — relevant
  to the TOCTOU note in §3.
- The actual e2e/integration test harness's capability to interrupt a
  streaming turn mid-flight deterministically (needed for checklist item 9)
  — I did not locate or read the reload e2e test files beyond
  `reload_tests.rs`'s existence as a sibling module declared at
  `reload.rs:512-514`.
- ~~Whether `text_content` gets saved to the session transcript after the
  mid-stream `graceful_shutdown` `break` (`turn_streaming_mpsc.rs:385`) or is
  silently dropped.~~ **Resolved during this pass**: the inner event loop
  (`loop { ... }` opened at `turn_streaming_mpsc.rs:363`) closes at
  `turn_streaming_mpsc.rs:925`, and execution unconditionally falls through
  to the "Add assistant message to history" block
  (`turn_streaming_mpsc.rs:1084-1122`), which pushes `text_content` into
  `content_blocks` if non-empty (`turn_streaming_mpsc.rs:1086-1090`) and
  calls `self.add_message_ext(...)` + `self.session.save()?`
  (`turn_streaming_mpsc.rs:1119-1122`) regardless of *why* the inner loop
  exited — normal stream end or the graceful-shutdown `break`. So `§3`'s
  `stash_interrupted_turn(&text_content)` call is checkpointing text that
  the transcript is *also* about to durably save via the same code path,
  moments later, in the same function. This is not redundant waste: the
  session transcript and the reload-recovery record serve different readers
  (the transcript is replayed as conversation history; the recovery record
  is read by `pending_directive_for_session` specifically to build the next
  turn's system-reminder). But it does mean `partial_content` in
  `TurnStashed`/`StashedTurnRecord` is derivable from the session transcript
  after the fact (the last assistant message, if the session's last known
  status was an interrupted running turn) as a fallback, which is worth
  keeping in mind if `stash_turn_for_session`'s write is ever lost (e.g.
  process killed harder than a graceful `execve`) — the transcript is the
  more durable of the two stores since `session.save()` has no dependency on
  `reload_recovery.rs`'s file existing or being well-formed.

DESIGN-A COMPLETE
