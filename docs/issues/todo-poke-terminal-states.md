---
status: open
priority: high
owner: maintainers
opened: 2026-07-31
---

# Todo subsystem: six different definitions of "finished", and `blocked_by` reaches none of them

Reported by human (2026-07-31), after an agent session spent ~170 consecutive
auto-pokes in a legitimately blocked state, followed by ~15 further turns that
looked idle but were the same loop running invisibly. The user's note:

> Hey, sorry about that - I thought that having turned "auto poke" off that
> would not have happened.

Turning auto-poke off was the right instinct and did not stop it. That is
finding 3 below.

Full working notes, including the verification transcript for each claim:
`~/.jcode/pending/todo-poke-no-blocked-state.md` (685 lines, 6 findings, 7
in-place corrections). This document is the durable summary.

## Symptom

An agent whose only remaining todo was genuinely blocked on a human decision
had no way to say so. Every turn ended with a synthetic nudge:

    You have 1 incomplete todo. Continue working, or update the todo tool.

Marking the item `completed` would have been false. Marking it `cancelled`
silenced the visible nudge but replaced it with a stream of **empty turns**.
Only a genuinely `completed` status reached a quiet state.

The unbounded nudge is not merely cosmetic. An unbounded "keep going" signal
applied to a correctly blocked agent selects for unnecessary work: in the
originating session it produced fifteen audit passes over already-verified
artifacts, one of which ended in an unauthorized write to live branch
protection. Overnight mode already bounds this (`OVERNIGHT_MAX_POKES = 48`,
`commands_overnight.rs:13`, plus a consecutive-no-progress stop); the ordinary
todo poke has no bound at all. That asymmetry is what makes it look like an
oversight rather than a design choice.

## Root cause: six sites, six different meanings of "finished"

Each site re-derives the predicate inline and they have drifted:

| # | site | "finished" means |
|---|------|------------------|
| 1 | `crates/jcode-app-core/src/tool/todo.rs:114-115` (hill-climbability nudge) | `completed` or `cancelled` |
| 2 | `crates/jcode-app-core/src/tool/todo.rs:132` (`remaining`, the tool-call title) | `completed` only |
| 3 | `crates/jcode-tui/src/tui/app/commands.rs:2558` (`is_incomplete_poke_todo`) | `completed` or `cancelled` |
| 4 | `crates/jcode-base/src/todo.rs:150-159` (nudge target selection) | `completed` only |
| 5 | `crates/jcode-tui/src/tui/app/commands.rs:2601` (`todo_confidence_summary`) | `completed` only, **and an empty set means "needs work"** |
| 6 | `crates/jcode-base/src/todo.rs:37-43` (`group_is_complete`) | `completed` only |

Sites 1 and 2 are adjacent functions eighteen lines apart in the same file.
Beyond these six, a precise grep (excluding tests) finds **12** production
sites spelling out `!= "completed" && != "cancelled"` inline, including four in
`crates/jcode-tui/src/tui/app/remote/key_handling.rs:2133,2199,2315,2381`.

Observable consequence of sites 2 vs 3, executed on a list of
`[completed, cancelled, pending]`:

    title shows        : "2 todos"
    poke/hill counts   : 1

The user is told more items are outstanding than any gate believes.

## Finding 1: `blocked_by` already exists, is settable, persists, renders, and no gate consults it

`blocked_by` is a field on the todo tool's own `TodoItem`
(`crates/jcode-task-types/src/lib.rs:222-223`):

```rust
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<String>,
```

It is **undocumented but reachable**. The tool schema
(`crates/jcode-app-core/src/tool/todo.rs:254`) never lists it, yet input is
deserialized with plain `serde_json::from_value` (`tool/todo.rs:330`) and there
is no `deny_unknown_fields` anywhere in `jcode-task-types`. An agent can set it
today by accident of that omission.

Verified end to end during the session: set by the agent, serialized, written
to disk, and read back by `load_todos` — which is the exact function the poke
path uses (`poke_todos`, `commands.rs:2554`, loads from disk on every poke, so
serialization survival was a real question rather than an obvious yes):

    7 | pending | blocked_by = ['user approval to write to live branch protection']

Five TUI sites already render a blocked indicator from it
(`ui_messages.rs:1341`, `info_widget_todos.rs:352,372`,
`todos_view.rs:331,577`, `turn_notify.rs:150,174`).

The poke ignores it entirely. Worse, the nudge text **structurally cannot**
mention blocking: `build_poke_message` (`commands.rs:2569`) receives only a
count, `build_auto_poke_message(incomplete.len())`, so no `blocked_by` data can
reach the string the agent is shown.

Live confirmation: `blocked_by` was set at 07:44:52 and pokes continued at
07:46:12, 07:47:41 and 07:48:43, unchanged in wording and cadence.

## Finding 2: the obvious one-line fix is wrong

"Exclude non-empty `blocked_by` from `is_incomplete_poke_todo`" would misfire,
and auditing the proposed fix before proposing it is how that was caught.

The recurring nudge comes from `schedule_auto_poke_followup_if_needed`
(`crates/jcode-tui/src/tui/app/input.rs:1262`), not from the `commands.rs` call
sites (both of which handle an empty list correctly). There,
`incomplete.is_empty()` is not a quiet stop; it is the entrance to a
**completion gate** (`input.rs:1277-1301`) which either pushes
`"🛑 Todo completion gate: ..."` and queues another message, or announces
`"✅ Todos complete."` and disarms auto-poke.

**Announcing "Todos complete" for a blocked item is a false statement**, and is
worse than the nudging it replaces: the nudge was annoying, this records the
wrong outcome.

Blocked is a *third* state — neither "incomplete, keep going" nor "complete,
celebrate" — so it cannot be expressed by editing one predicate.

## Finding 3 (why turning auto-poke off did not help): an all-`cancelled` list traps the confidence gate

`todo_confidence_summary` (`commands.rs:2600`) counts only
`status == "completed"`. For a list whose sole item is `cancelled`, that set is
empty, so:

```rust
    let needs_more_work = completion_average
        .map(|avg| avg < TODO_CONFIDENCE_THRESHOLD)
        .unwrap_or(true)
```

`completion_average` of an empty set is `None`, `unwrap_or(true)` fires, and
`needs_more_work` is true. Back in `schedule_auto_poke_followup_if_needed`,
`incomplete.is_empty()` is now also true, so it enters the completion gate,
sees `needs_more_work`, pushes a **hidden** system message and sets
`pending_queued_dispatch = true`. A turn is dispatched with no visible poke
text — an **empty turn** — and because the todo list never changes, it repeats.

So `cancelled` does not reach a quiet state. It silences the visible nudge and
hands the loop to the confidence gate, which is *less* informative, not more.
`format_todo_completion_confidence` would label it `"unknown"`.

The dangerous property is that an empty input yields "needs more work" rather
than "nothing to assess".

## Finding 4: a rejected todo update is discarded, and the message names the wrong cause

An update that trips `newly_completed_groups_have_sufficient_ownership`
(`crates/jcode-base/src/todo.rs:48`) is discarded **across all groups**, and
the tool response echoes the *previous* list — indistinguishable from success
unless the caller reads the result back and diffs it.

A signal does exist (`observe.rs:194-208` emits
`🛑 Todo completion gate: end-to-end ownership needs full-outcome
follow-through` plus `TODO_OWNERSHIP_CONTINUATION_MESSAGE`), so this is not
silent. The narrower defect is that **neither message says the update was
discarded, nor names the offending group**.

Worse, observed live: the emitted message was *"Your end-to-end ownership is
not high enough to complete this goal"* when ownership was **97** against a
threshold of **96**. The true fault was a **group-label mismatch** — the todo's
group was `"PR #59 governance window"` while the goal carrying the assessment
was labelled `"Remove launch hotkeys (PR #59)"`, so this lookup returned
`None`:

```rust
        goals
            .iter()
            .find(|goal| normalized_group(goal.group.as_deref()) == group)
            .and_then(|goal| goal.end_to_end_ownership)
            .is_some_and(|score| score >= QUALITY_GATE_THRESHOLD)
```

The message cannot say so: it is a fixed constant
(`crates/jcode-base/src/todo.rs:23`) with no access to which group failed or
why. An agent acting on it will keep rewriting a perfectly good assessment
instead of fixing a typo in a label. Three distinct faults — *no matching
goal*, *no score*, and *score below threshold* — currently share one sentence,
and it describes only the third.

## Suggested shape of a fix, cheapest first

1. **Report the real cause** (`TODO_OWNERSHIP_CONTINUATION_MESSAGE` and the
   `observe.rs` emission). Say the update was **discarded**, name the group,
   and distinguish missing-goal from missing-score from below-threshold. This
   is a string/plumbing change, independently useful, and cannot break
   behaviour.
2. **One shared predicate** for "this item is terminal", in `jcode-base`, used
   by all six sites. A pure refactor whose only behaviour change is making the
   `remaining` title agree with every gate — which is itself the bug fix.
3. **Give `todo_confidence_summary` a distinct empty-input case.** `None` must
   mean "no completed items to assess", not "needs more work".
4. **Teach the poke about `blocked_by`.** Partition three ways in
   `schedule_auto_poke_followup_if_needed`:

```rust
    let (blocked, incomplete): (Vec<_>, Vec<_>) = todos
        .iter()
        .filter(|todo| super::commands::is_incomplete_poke_todo(todo))
        .cloned()
        .partition(|todo| !todo.blocked_by.is_empty());
```

   (`.cloned()` must precede `.partition()`: `build_poke_message` takes
   `&[TodoItem]`, and `input.rs:1272-1276` already clones for this reason.)
   Then: non-empty `incomplete` pokes as today; empty `incomplete` with
   non-empty `blocked` takes a **new branch** that stops poking and displays
   something true (`⏸ Blocked on: <reasons>`), explicitly **not** falling
   through to the completion gate; both empty keeps the existing gate.
5. **Pass items, not a count**, to `build_poke_message` so the nudge can name
   what is blocking.
6. **Document `blocked_by` in the tool schema** so it is discoverable rather
   than accidental.
7. **Bound the ordinary poke**, matching `OVERNIGHT_MAX_POKES`, as a backstop
   independent of all of the above.

Order matters: (1) is safe alone; (2) and (3) must land before (4), or the new
blocked branch inherits the drift it is meant to escape.

## Not inspected

* The remote/headless poke path beyond confirming `is_incomplete_poke_todo` has
  four call sites, all in `jcode-tui`. `remote/key_handling.rs` duplicates the
  predicate inline four times.
* Whether any existing test pins the current two-terminal-status behaviour,
  which would size the blast radius of consolidating it.
* The desktop app (`session_launch.rs:156` has an unrelated `Cancelled`
  variant); out of scope for TUI/CLI.

## R08(h): a poke reported a superseded count as present tense (2026-08-02)

Observed live in `session_badger_1785586811874_613d6cdce5daf938`, on the binary
built 2026-08-01 16:39 (`~/.jcode/current/jcode`, which predates the R08(b)/(c)
fixes and does not contain `is_terminal_todo_status`).

At `06:58:22.135775Z` the auto-poke said **"You have 4 incomplete todos."**
The on-disk list had held **2** incomplete since `06:58:03.792681Z`, a lag of
**18.2 seconds**, and the `todo` tool_result at `06:58:04Z` independently
confirms the written statuses as `[completed, completed, completed, pending,
pending]`. So the correct value existed, in the file the poke reads, for 18
seconds before the poke asserted a different one.

This is the same failure R08 is about, in a new costume: the count is not
merely wrong, it is a *previous revision's* count presented as current. `4` was
the true count of the revision written at `05:59:21Z`.

Checked across all three pokes in the session:

| poke (UTC) | said | file held at that moment | verdict |
| --- | --- | --- | --- |
| `05:09:14` | 4 | 4 (written `05:08:57`) | correct |
| `05:59:05` | 4 | 4 (written `05:08:57`) | correct |
| `06:58:22` | 4 | **2** (written `06:58:03`) | **WRONG** |

### Hypotheses tested and falsified

Recording these because each is individually plausible and a reader would
otherwise retry them:

1. **Stale terminal-status rule** (the pre-R08(b) `status != "completed"`
   filter). Falsified: applied to the current file, *both* the old and new
   rules yield 2, not 4. The running binary is genuinely old, but that does not
   explain this number.
2. **Message frozen at queue time.** `build_poke_message` renders a `String`
   into `queued_messages`, and the dispatcher does `std::mem::take` and
   forwards it verbatim, so this *is* a real structural gap. But it does not
   explain this instance: the todo write landed 18s before the poke and the
   poke is scheduled at `finish_turn`, after the write.
3. **`TODOS_CACHE` staleness.** There is a time-based todos cache in
   `helpers.rs` whose only production invalidation is in `observe.rs:112`. But
   the poke path calls `crate::todo::load_todos` directly and never reads that
   cache, so it is not implicated.
4. **`.bak` recovery.** `~/.jcode/todos/<id>.bak` does contain exactly the
   4-open revision from `05:59:22Z`. Falsified anyway:
   `read_json_with_recovery_handler` consults `.bak` only when the primary
   fails to parse, the primary parses fine (5 items), and the two files have
   distinct inodes and link counts of 1.

### Status: OPEN, root cause not established

Four hypotheses, four falsifications. I am recording this rather than
continuing, because the honest state is that I cannot yet name the mechanism,
and the next plausible-sounding guess would be the fifth. What is *established*
is the observation itself, which is reproducible from the stored transcript and
the file timestamps, and which no amount of further theorizing changes.

Worth noting for whoever picks this up: hypothesis 2 describes a genuine gap
(the count is rendered at schedule time and never recomputed at send time) that
should probably be closed on its own merits even though it does not explain
this instance. A poke that recomputed at dispatch would be immune to the whole
class, whatever the specific mechanism here turns out to be.

The first step should be reproducing on a *current* binary. This session's
binary predates the R08 fixes, so the behavior may already differ.

### Update: the poke self-corrected (2026-08-02T07:11:22Z)

The next poke, one turn later, said **"You have 2 incomplete todos"** — the
correct count, against an unchanged file. So the defect is **intermittent and
self-clearing**, not a persistent wrong reading. Any explanation must account
for a stale value that later resolves on its own with no intervening write.

That single fact retires most of the remaining candidates: whatever produced
`4` was transient state inside the running process, not a wrong rule, not a
wrong file, and not a wrong session id.

Three further hypotheses tested after the self-correction, all falsified:

5. **Reload replaying a queued string.** No reload occurred between the
   `05:59` revision and the bad poke.
6. **A storage-layer read cache or deferred write.** Neither exists.
   `read_json` has no cache (zero `LazyLock<Mutex>`/`CACHE` in
   `crates/jcode-storage/src/lib.rs`) and `write_json_fast` is a temp-file +
   atomic rename, so the write is visible the instant it returns.
7. **Writer and poke disagreeing on session id.** Falsified: the write path
   (`active_client_session_id`, `state_ui.rs:81`) and the poke path
   (`active_session_id`, `commands.rs:2537`) resolve identically, both
   preferring `remote_session_id` when remote and `session.id` otherwise.

Seven hypotheses, seven falsifications. I am not proposing an eighth. The
remaining shape is in-process transient state on the TUI side, which I cannot
narrow further from a stored transcript, and this session's binary predates
the R08 fixes anyway.

**Recommended next step, unchanged and now better motivated:** close the
schedule-time/send-time gap (hypothesis 2) on its own merits. It is a real
structural defect regardless of whether it caused this instance, and a poke
that recomputes its count at dispatch is immune to the entire class of
transient-state explanations that remain — including whichever one this
actually was.

### Resolution: the schedule-time/send-time gap is closed (2026-08-02)

Implemented the recommendation above. The count is now re-derived when the
queue is drained, not when the poke is queued.

- `commands::refresh_poke_message_for_dispatch` re-reads the todo list and
  rebuilds the message. Non-poke messages pass through untouched, and a poke
  whose todos all resolved in the meantime returns `None` and is dropped rather
  than sent announcing "0 incomplete todos".
- `App::take_queued_messages_for_dispatch` drains the queue through that
  refresh. All three dispatch sites use it: `input.rs` `process_queued_messages`
  and both `remote.rs` drains.
- `input.rs:1194` (`retrieve_pending_message_for_edit`) is deliberately **not**
  refreshed. It pulls queued text into the user's input box for editing rather
  than sending it to the model, so silently rewriting or dropping a message
  there would destroy something the user asked to edit.

This does **not** claim to fix the observed instance, whose root cause remains
unknown. It closes the structural gap that produces the same symptom.

Four tests in `state_model_poke_04.rs`, each falsified before being trusted:

1. `poke_message_is_rebuilt_from_the_todo_list_at_dispatch_time` — queue 4,
   resolve 2, expect 2.
2. `poke_refresh_preserves_the_real_count_when_todos_are_unchanged` — the
   contrast case, so a "fix" that merely dropped the number cannot pass.
3. `poke_refresh_leaves_user_messages_alone_and_drops_emptied_pokes`.
4. `draining_the_queue_for_dispatch_refreshes_poke_counts_in_place` — the
   wiring control.

Test 4 exists because of a near-miss worth recording. Tests 1-3 call the
refresh helper directly, so they all still passed when the call was deleted
from `process_queued_messages`: the entire 1874-test suite stayed green against
a completely unwired fix. `process_queued_messages` needs a live terminal and
event stream, which is why the drain was extracted into
`take_queued_messages_for_dispatch` — a seam a test can reach. Test 4 was then
confirmed to fail with the refresh unwired and pass with it wired.

### Incidental finding: `benchmark_resume_loading_reports_timings` is flaky

Surfaced while validating the above; **unrelated to pokes and pre-existing**.

`session_picker::loading::tests::benchmark_resume_loading_reports_timings`
writes 120 sessions and asserts `load_sessions()` returns >= 100. Under CPU
contention it intermittently returns fewer — observed **24** and **87** — about
1 run in 8, and never on an idle machine.

Attribution was established by control rather than assumption: the branch
failed 3 times in 16 runs while the baseline passed 16 for 16, which *looks*
like a regression. Running the **unmodified baseline under identical load**
reproduced the failure (`count=87`), so the poke change is exonerated and the
flake predates it. Load, not the diff, was the hidden variable.

Not fixed here, as it is outside this node. Two observations for whoever takes
it: `reset_tui_test_globals` does not invalidate the session-list cache (unlike
the neighbouring tests, this one never calls `invalidate_session_list_cache()`
before loading), and `session_load_thread_count` derives its width from
`available_parallelism`, which contention can change. Either could plausibly
produce a short count; neither is confirmed, and I did not test them.

The bare `assert!` was replaced with one that prints the observed count and the
relevant env vars, since the original failed with no information at all.
