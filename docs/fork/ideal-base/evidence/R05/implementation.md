# R05 implementation evidence

Node: R05 (implement), wave W4. Branch `automation/w4-r05`, based on main
`eee5ccc71`.

Companion files in this directory:

- `README.md` — the original incident writeup (2026-07-20).
- `incident-log-excerpt.txt` — the raw daemon log excerpt.

This file records what was changed, why, and how each change was proven
non-vacuous.

## Scope outcome

| Sub-issue | Status |
| --- | --- |
| (a) duplicate-attach policy | Fixed |
| (b) queued-duplicate collapse | Fixed |
| (c) truthful stall-guard cancel label | **Not fixed — blocked outside owned paths** |
| (d) working_dir integrity on reconnect | Fixed |

## (a) Dual attach was silent

**Race.** `handle_resume_session` computes
`can_take_over_live_session = allow_session_takeover && client_has_local_history
&& !distinct_client_instances` (`crates/jcode-app-core/src/server/client_session.rs`
around line 1160). When two *distinct* client instances collide, takeover is
deliberately refused, because the existing owner is a live client that may be
mid-turn. The non-live branch reports that refusal to the joiner; the **live
attach branch fell through silently**, leaving two clients attached with neither
told. Each then ran its own stream-stall guard, and each guard's cancel killed
the other's turn (incident log, 11:43:15 through 11:43:28: alternating
`request_kind=cancel` from `conn_1784527628584_…` and `conn_1784556373101_…`
against one `session_fish_…`).

**Invariant established.** A refused takeover is never silent: every client
attached to the session receives a dual-attach warning.

**Design decision (deliberate).** Takeover is *still refused*. The alternative
in the node text ("newest client wins with explicit takeover banner") would
disconnect a distinct, live client instance that may be mid-turn, destroying
legitimate work — the same class of harm as the incident itself. The acceptance
gate permits either remedy ("or at minimum a prominent 'N clients attached'
warning on both"); the warning is the safe one. Rationale is recorded in the
code comment at the fix site.

**Change.** `client_session.rs`: `dual_attach_conflict` captures the refused
conflict; after `register_session_event_sender`, a `ServerEvent::Notification`
is fanned to all attached clients via `fanout_live_client_event`, alongside
`logging::warn` and `logging::event_warn("SESSION_LIFECYCLE",
phase=dual_attach_warned, …)`.

## (b) Recovery replayed already-recovered messages

**Race.** `recover_stranded_soft_interrupts`
(`crates/jcode-tui/src/tui/app/remote/queue_recovery.rs`) unconditionally
prepended recovered interrupts into `app.queued_messages`. Under repeated
interrupts — exactly what (a) produces — recovery re-ran over soft interrupts it
had already moved into `queued_messages` but not yet dispatched, appending
another copy each time. Incident log 11:43:15.992: "Preserving 2 pending soft
interrupt(s) across remote error" / "Recovering 2 stranded soft interrupt(s)
into queued follow-ups after turn boundary", repeating until 18 duplicate
deliveries accumulated.

**Invariant established.** Recovery never introduces a copy of a message already
waiting in the queue. N recovery cycles over the same content leave exactly one
queued copy.

**Collapse key, and why it is safe.** The dedup keys on **recovery provenance
only**: a recovered interrupt is dropped solely because an identical copy is
already in `queued_messages` or already in the batch being recovered. Messages
the user genuinely typed twice reach `queued_messages` through the input path,
never through recovery, so they are never candidates for filtering and still
deliver twice. This is asserted directly by
`user_typed_duplicates_are_not_collapsed`.

## (c) BLOCKED — stall-guard cancels are indistinguishable from reloads

This is **not** a missing-label bug, and the node's premise is factually wrong.

The node states "trigger=stall_guard is already in the interrupt metadata". It
is not. The wire request is `Request::Cancel { id: u64 }`
(`crates/jcode-protocol/src/wire.rs:143`) and carries **no reason field**. The
stall-guard trigger exists only in *client-side* log fields
(`crates/jcode-tui/src/tui/backend.rs:399-408`, `interrupt_request_log_fields`)
and is never transmitted. Confirmed by the incident log itself: every stalled
handler line records `request_kind=cancel` with no trigger field.

The label is selected in `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:1608`
via `self.is_graceful_shutdown()`. That reads the agent's `graceful_shutdown`
signal — and `SessionControlHandle::new` is constructed with
`agent.graceful_shutdown_signal()`
(`crates/jcode-app-core/src/server/client_lifecycle.rs:585`), so
`SessionControlHandle::request_cancel` (`crates/jcode-app-core/src/server/state.rs:614`)
fires **the same `InterruptSignal` instance** a server reload fires. At the point
of labeling, a user cancel and a server reload are literally the same bit.

A truthful fix therefore requires one of:

1. A reason/kind field on `Request::Cancel` in
   `crates/jcode-protocol/src/wire.rs` (**not owned by R05**), plumbed to the
   label site; or
2. Separating the cancel signal from the graceful-shutdown signal in
   `crates/jcode-app-core/src/agent.rs` (field `graceful_shutdown`, line 266),
   `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs`,
   `crates/jcode-app-core/src/agent/turn_loops.rs`, and
   `crates/jcode-app-core/src/server/state.rs` (**none owned by R05**).

R05 owns `client_session.rs`, `client_lifecycle.rs`, `agent/interrupts.rs`, and
`tui/app/remote/**`. `agent/interrupts.rs` only exposes the shared signal
(`graceful_shutdown_signal`, `is_graceful_shutdown`); it does not choose the
label and cannot distinguish the two causes without the upstream change.

No cosmetic relabel was attempted. Renaming the string in an owned file without
a real cause signal would make the message *differently* wrong rather than
truthful, which is worse than the current honest-but-wrong label because it
would silence the incident's own diagnostic trail. Escalated to the coordinator
instead.

## (d) Reconnect silently rescoped working_dir

**Race.** `handle_subscribe` (`client_session.rs`, around line 489) called
`agent_guard.set_working_dir(dir)` unconditionally on every Subscribe, and
`provider.rs:220` overwrites and saves without comment. A reconnecting client
moved a 13-hour-old session from `/Users/jrudnik/labs/jcode` to `/Users/jrudnik`
with no user-visible trace, silently changing swarm identity and every relative
path the session resolved.

**Invariant established.** A Subscribe that changes an established `working_dir`
emits a warn log, a `SESSION_LIFECYCLE` `subscribe_working_dir_changed` event
naming both directories, and a client-visible notification. A Subscribe that
does not change it stays silent.

**Design decision.** The change is **still applied**. Refusing it would strand a
client that legitimately reopened the session elsewhere against a stale
directory. The acceptance gate's second clause ("or must at minimum surface the
change loudly") is the one satisfied.

## Validation

All commands run from the worktree with `JCODE_REMOTE_CARGO=0` (the default
remote builder cannot resolve this worktree path).

### Tests added

| Test | File |
| --- | --- |
| `handle_resume_session_warns_both_clients_on_refused_takeover` | `crates/jcode-app-core/src/server/client_session_tests/resume/dual_attach_warning.rs` |
| `subscribe_warns_when_reconnect_changes_established_working_dir` | `crates/jcode-app-core/src/server/client_session_tests/subscribe_working_dir.rs` |
| `subscribe_is_quiet_when_working_dir_is_unchanged` | `crates/jcode-app-core/src/server/client_session_tests/subscribe_working_dir.rs` |
| `recovery_does_not_requeue_a_message_already_queued` | `crates/jcode-tui/src/tui/app/remote/queue_recovery.rs` |
| `repeated_recovery_cycles_do_not_accumulate_duplicates` | `crates/jcode-tui/src/tui/app/remote/queue_recovery.rs` |
| `user_typed_duplicates_are_not_collapsed` | `crates/jcode-tui/src/tui/app/remote/queue_recovery.rs` |

### Passing runs

    scripts/dev_cargo.sh test -p jcode-app-core --lib -- resume_tests
    → 8 passed; 0 failed (includes the new dual-attach test and the
      pre-existing handle_resume_session_allows_attach_from_different_client_instance,
      which still passes: the joiner is still allowed to attach)

    scripts/dev_cargo.sh test -p jcode-app-core --lib -- subscribe_working_dir
    → 2 passed; 0 failed

    scripts/dev_cargo.sh test -p jcode-tui --lib -- queue_recovery
    → 3 passed; 0 failed

### Non-vacuity proofs (DECISIONS.md D029)

Each fix was individually neutralized, the test re-run to observe failure, then
the fix restored and the test re-run to observe the pass.

**(a)** Neutralized by forcing `dual_attach_conflict` to `None` at the fix site.

    test …handle_resume_session_warns_both_clients_on_refused_takeover ... FAILED
    joining client must be warned about the dual attach, got [Done { id: 77 }]

The bare `Done` is exactly the silent attach the incident describes.

**(b)** Neutralized by short-circuiting the dedup predicate to `false`.

    test …repeated_recovery_cycles_do_not_accumulate_duplicates ... FAILED
      left: ["keep going", × 18]
     right: ["keep going"]
    test …recovery_does_not_requeue_a_message_already_queued ... FAILED
      left: ["retry the failing test", "retry the failing test"]
     right: ["retry the failing test"]
    test …user_typed_duplicates_are_not_collapsed ... ok

18 recovery cycles produced exactly 18 copies, reproducing the incident's
observed 18x duplicate delivery. The user-typed-duplicates test passing in both
configurations is the intended result: it guards the *absence* of
over-collapsing, so the fix must not change its outcome.

**(d)** Neutralized by adding `&& false` to the change-detection condition.

    test …subscribe_warns_when_reconnect_changes_established_working_dir ... FAILED
    reconnect working_dir change must notify the client, got [Done { id: 1 }]
    test …subscribe_is_quiet_when_working_dir_is_unchanged ... ok

Again the quiet-path test passes in both configurations by design: it guards
against a warning that fires on every reconnect.

All probes were reverted; the committed tree contains no probe residue.

## Files changed

    crates/jcode-app-core/src/server/client_session.rs                        (a), (d)
    crates/jcode-app-core/src/server/client_session_tests.rs                   test wiring
    crates/jcode-app-core/src/server/client_session_tests/resume.rs            test wiring
    crates/jcode-app-core/src/server/client_session_tests/subscribe_working_dir.rs  new
    crates/jcode-app-core/src/server/client_session_tests/resume/dual_attach_warning.rs  new
    crates/jcode-tui/src/tui/app/remote/queue_recovery.rs                     (b) + tests

### Ownership note

`crates/jcode-app-core/src/server/client_session_tests*` is not literally listed
in R05's `owned_paths`; those paths name `client_session.rs` itself. The test
files are the test module of an owned file (`client_session.rs` declares them via
`#[path = "client_session_tests.rs"] mod tests;`) and there is no way to test the
owned behavior without them. No other W4 node owns any `client_session` path
(verified against `WORK_GRAPH.json`); the only other claimant is R01 under W1,
which is `accepted`. Flagged rather than assumed.
