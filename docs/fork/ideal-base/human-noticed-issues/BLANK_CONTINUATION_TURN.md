# Empty user turns reach the provider because hidden continuations persist a blank text block

Reported by human (2026-08-01): messages appearing in the transcript as if the
user had sent nothing, each burning a full model call. Observed 10 times in the
reported session; a later fleet-wide scan found **1348 across 2398 sessions**,
of which 1241 are runaway loops (see "Scope is larger than the report" below).

**Fixed** in PR #81 ("fix(agent): stop sending blank user turns on hidden
continuations") — but only the blank *content*. PR #81 does not stop the loops
that produce most of them. This document records the diagnosis, and in
particular records a *wrong first fix that was merged*, because the failure mode
of that fix is the durable lesson.

## Symptom

The transcript contains user turns stored as a literal empty text block:

```json
{"role": "user", "content": [{"type": "text", "text": ""}]}
```

Each one starts a real turn: a provider request is issued, tokens are spent, and
the model answers a message the user never sent. They cluster after reloads and
after the assistant finishes a reply while idle.

## Root cause

The hidden-continuation path deliberately sends **empty content plus a
side-channel `system_reminder`**:

```rust
// crates/jcode-tui/src/tui/app/remote.rs
begin_remote_send(app, remote, String::new(), vec![], true, Some(combined), true, 0)
```

The reminder is not part of the user message. It is applied to the system prompt
via `current_turn_system_reminder`, so an empty body is intentional and correct
on the wire. The defect is one layer down: `run_once_streaming_mpsc`
(`crates/jcode-app-core/src/agent/turn_execution.rs`) appends the user text block
unconditionally, so an empty `user_message` becomes a persisted, provider-visible
empty turn.

Two producers queue these continuations:

* reload recovery (`remote/server_events.rs`), after an interrupted turn
* the auto-poke todo gate (`input.rs`), while idle

Note that the *runaway loops* documented below are a third path and are **not**
the auto-poke; see "Remaining defect".

## Evidence

Three independent layers agree, which is why the diagnosis is not inferential:

| layer | signal |
|---|---|
| wire (`SERVER_REQUEST_LIFECYCLE`) | `content_bytes=0 content_chars=0 request_kind=message` |
| evidence log (`*.evidence.jsonl`) | `turn_started` with `sha256=e3b0c44298fc…` — the empty-string digest |
| session snapshot | `{"type":"text","text":""}` at the matching message indices |

Counting these needs care. A naive text search for `"text":""` over the session
file overcounts: it also matches empty strings *nested inside a `tool_use`
input*, which are not message bodies at all. The defect is a message whose
content is exactly one empty text block, and by that definition there are 10 in
the reported session.

## The first fix was wrong, and its wrongness is the point

PR #79 (`436f63c0a`, "fix(tui): never dispatch a blank user turn") added a
`trim().is_empty()` guard in `submit_input`. It was merged, deployed via
selfdev reload into the very session that was reproducing the bug, and **the bug
immediately reproduced again on the fixed binary**.

The guard sits on the *typed-input* path. These turns never touch it: they
originate from `begin_remote_send` with a programmatically empty body. The
running binary contained the guard, and the guard's log line never fired —
a directly falsifying observation, available within one turn of shipping.

The guard does close a genuine, unrelated gap: the entry guard at
`input.rs` tests `!input.is_empty()` rather than `trim()`, so typing a space and
pressing Enter dispatched a blank turn. That is worth keeping. Only the commit
message is wrong: it claims to fix the empty user messages, and it does not.

The lesson is narrow and general: a plausible mechanism that explains the
symptom is not the mechanism. The available direct evidence (`content_bytes=0`
on the wire, pointing at a programmatic sender rather than a keyboard) was
present *before* the first fix was written and was not consulted.

## Constraint on the fix, and how it was resolved

The obvious repair — skip the text block when the body is empty — is not
sufficient on its own, because it produces a user message with **zero content
blocks**, and:

```rust
// crates/jcode-provider-anthropic/src/lib.rs
let content = format_content_blocks(&msg.content, is_oauth);
if !content.is_empty() {          // zero-block message is dropped entirely
    result.push(ApiMessage { role: role.to_string(), content });
}
```

A dropped message can leave the transcript **ending on an assistant message**.
For Anthropic that is prefill/continuation semantics: the model continues the
previous assistant message instead of starting a new turn. No guard against a
trailing assistant message exists in `format_messages`.

Checked against the 10 real occurrences rather than argued in the abstract:

| preceding message | indices | count | outcome if the blank is skipped |
|---|---|---|---|
| user (`tool_result` ×4, text ×1) | 75, 135, 220, 717, 948 | 5 | safe — merged by the same-role pass |
| assistant text | 731, 735, 739, 743, 813 | 5 | transcript ends on assistant → prefill risk |

The split maps onto the two producers: reload-recovery continuations follow a
`tool_result` and are safe; the idle/auto-poke continuations follow assistant
text and are not. The risky half is one contiguous cluster, as expected for a
repeated idle poke.

So the empty text block was accidentally load-bearing: it was what terminated
the assistant turn. The shipped fix therefore branches on the trailing role
rather than skipping unconditionally:

* transcript ends on a **user** message → drop the message entirely; the
  reminder still reaches the model through the system prompt.
* transcript ends on an **assistant** message → keep one user message and carry
  the reminder text as its body, terminating the assistant turn without
  inventing a blank message.

A zero-block message is never stored, because providers reject an empty content
array.

## Scope is larger than the report

The reported session had 10 blanks. Parsing every session in `~/.jcode/sessions`
found **1348 blanks across 2398 sessions**, and they are not evenly spread:

| blanks | longest run | date | session |
|---|---|---|---|
| 600 | 290 | 2026-07-29 | `blossom_1785304954778` |
| 361 | 165 | 2026-07-30 | `piglet_1785442483326` |
| 194 | 144 | 2026-07-29 | `tulip_1785304996653` |
| 71 | 26 | 2026-08-01 | `retriever_1785438764739` |
| 15 | 14 | 2026-07-23 | `rabbit_1784775595418` |
| 10 | 1 | 2026-08-01 | the reported session |

"Longest run" counts blanks spaced exactly two apart, i.e. blank → assistant →
blank. **1241 of the 1348 sit inside such runs**, concentrated in five sessions.
The shape is a pump:

```
505 user      ""
506 assistant "Idle."
507 user      ""
508 assistant "Idle."
     … 290 consecutive rounds …
```

The reported session, with its longest run of 1, was among the mildest cases.
Diagnosing from it alone made the defect look like a content bug affecting ten
messages rather than a control-flow bug worth hundreds of model calls.

## What PR #81 does and does not fix

Replaying the shipped guard over the worst transcript separates the two:

| | before | after guard |
|---|---|---|
| blank user turns | 600 | **0** |
| provider-visible user turns | 860 | **858** |

The guard is correct and does exactly what it claims: no blank content, no
trailing-assistant prefill. But the loop's blanks nearly all follow assistant
text, so they take the *promote* branch rather than the *drop* branch. **598
model calls still dispatch.** The turns stop being blank; they do not stop being
turns, and the cost to the user is essentially unchanged.

So the empty text block was a *symptom* of the loop, not its cause. Fixing the
symptom made the transcript well-formed while leaving the expense intact.

## Remaining defect: an unbounded hidden-continuation pump

The loop is **not** the auto-poke, which is what I first assumed. The evidence
rules it out: every user message in `blossom`'s 600-blank region is blank. An
auto-poke would read "You have N incomplete todos. Continue working…" as its
body, because `build_poke_message` returns unbracketed text that
`partition_queued_messages` routes to `user_messages`, not to the reminder.

What the loop actually looks like is a `hidden_queued_system_messages` pump.
That branch (`remote.rs`) sends `String::new()` with the payload as a reminder,
which is exactly the blank-body shape observed:

```
} else if !app.hidden_queued_system_messages.is_empty() {
    let combined = reminders.join("\n\n");
    begin_remote_send(app, remote, String::new(), vec![], true, Some(combined), true, 0)
```

The assistant side is 545 replies of literally `"Idle."`, so each round costs a
model call to say nothing. Something re-enqueued a hidden reminder after every
completed turn without a termination condition. The mid-turn recovery reminder
(`response_recovery.rs`) is *not* the culprit: it appears only 19 times and is
explicitly bounded by `MAX_INCOMPLETE_CONTINUATION_ATTEMPTS`.

### Confirmed live, in the session that wrote this document

The re-enqueue question was settled by an accidental self-repro: the agent
writing this writeup reproduced **both** defects in its own session.

```
blank turns in that session   : 2   (indices 3 and 7, both prev=assistant)
auto-poke turns               : 6   (indices 123, 183, 292, 318, 326, 350)
gaps between pokes            : 60, 109, 26, 8, 24
```

Both halves are visible at once, and they are clearly *different* producers:

* The two blanks are the hidden-continuation shape (empty body, payload in the
  reminder, previous message an assistant reply) — the defect PR #81 fixes.
* The six pokes each carry a real body, `"You have 1 incomplete todo. Continue
  working, or update the todo tool."`, and recurred with **no cap** while the
  todo list stayed incomplete.

This is the direct confirmation that the auto-poke is unbounded *and* that it is
not the source of the blanks — the two coexist in one transcript without
interacting. A poke never appears as a blank, and a blank never carries poke
text.

It also explains the `blossom` shape: a bounded-looking agent loop and an
unbounded reminder pump can drive the same session from opposite ends.

### Narrowing the pump: every text-carrying producer is excluded

Each candidate was tested the same way — take the producer's *literal output
string* from the source and count it in the transcript. A producer responsible
for 600 rounds must appear ~600 times.

| candidate | its output text | occurrences in `blossom` |
|---|---|---|
| auto-poke (`input.rs`) | `"You have N incomplete todos..."` | **1** |
| todo confidence gate | `TODO_COMPLETION_CONTINUATION_MESSAGE` | **0** |
| overnight poke | `"Overnight auto-poke for run ..."` | **2** |
| reload recovery (x2 sites) | `ReloadContext` continuation | **0** |
| `response_recovery.rs` | `"[System reminder: your previous..."` | 19 (bounded) |

None of them can account for 600. The display side agrees: the string
`"Auto-poking:"`, which `input.rs` pushes *every* time it queues a poke,
appears **zero** times in the whole file.

### The blanks carry no payload at all

This is the part that overturns my earlier writeup. A blank turn looks like:

```json
{"role":"user","content":[{"type":"text","text":""}]}
```

There is no `system_reminder` and no hidden text — nothing rides along. And the
model's own accounting confirms the provider saw nothing:

```
input_tokens across the 600 loop replies:  min 22   p50 22   max 1318
```

**22 input tokens.** A hidden-continuation reminder is hundreds of tokens; the
median call carried essentially an empty new turn against a warm cache
(`cache_read_input_tokens` ~137950). So the loop is **not** a reminder being
re-enqueued. I recorded `hidden_queued_system_messages` as the "matching
shape"; the token accounting says otherwise, and I was pattern-matching on
`String::new()` rather than checking what was actually sent.

The re-fire is also far too fast to be deliberative: median **137 ms** between
the assistant's reply and the next blank, sustained for **124 minutes** across
1264 messages, answered 547 times with literally `"Idle."`.

### What that leaves

The token accounting pins the loop down precisely. Per round, across 600 rounds:

```
input_tokens        22      (constant)
output_tokens        6      (constant — the string "Idle.")
cache_read_input  +26 per round, monotonically
```

The 26-token growth is exactly a blank user turn plus an `"Idle."` reply being
appended to history. So the request is well-formed and the history is intact —
the model is simply being asked to respond to **nothing**, 600 times, and
correctly reports that it has nothing to do.

**What the 22 tokens actually are** (found by @badger with a wire-boundary
assertion, correcting my phrasing): a stored `""` does *not* arrive at the
provider empty. `Message::with_timestamps` prefixes every user text block at
send time, so `""` becomes `"[2026-08-01T19:25:02.874Z] "`. The 22 tokens are
that timestamp tag and nothing else. "Empty on the wire" was imprecise; the
turn carries its own timestamp and no content. This is also why the defect is
easy to miss at the boundary — an `is_empty()` assertion there can never
fire.

A control rules out a caching artifact. In the *same* session with the *same*
warm cache, the 19 replies that follow a real `response_recovery` reminder cost
**90** input tokens; the 830 that follow a blank cost **22**. A reminder is
~4x. There was no reminder.

`blossom`'s todo file is the standing condition: **6 todos, 5 pending and 1
in_progress, none ever completed.** `schedule_auto_poke_followup_if_needed`
therefore returns `true` on every single turn completion, forever — it is
called from four post-turn sites (`local.rs:502`,
`server_events.rs:1055/1169/1385`) and has no repeat bound and no comparison
against the previous poke.

That is the *driver*. But the poke it queues carries text, and no poke text
appears in the transcript, so the queued body is being lost between enqueue and
send. Both dispatchers `mem::take` the queue and then send `messages.join()`;
if the queue is drained by another handler in between, `combined` is empty and
an empty turn ships. The display marker `"Auto-poking:"` never appearing while
the poke *is* firing is consistent with the enqueue being lost, not skipped.

**That hypothesis is now dead.** I named its disproof — "if the poke text
reaches the wire intact, the desync theory is dead" — and then ran it. Against
the real `jcode-base` code:

```
n=1  -> "You have 1 incomplete todo. Continue working, or update the todo tool."
n=6  -> "You have 6 incomplete todos. Continue working, or update the todo tool."
```

The body is never empty and never bracketed, so `partition_queued_messages`
routes it to `user_messages` and it ships as a real user body. Nothing is lost
between enqueue and send. **The 600 blank turns were never pokes at all**, and
the unbounded poke gate — while genuinely unbounded — is not what produced
them. That is the third suspect this investigation has had to discard, and the
second one I discarded by running the test I had written down rather than by
arguing from the code.

### The blanks are a separate, wider defect

Two facts reframe it:

* `blossom` ends with **three consecutive user turns** and
  `status: Crashed (process exited, no shutdown signal)`. Strict alternation
  breaks at the end, so something was appending user turns with no reply.
* The same signature appears in `piglet` with `is_debug: false`, no self-dev
  build, and `status: Closed` — a **normal session that ended cleanly**:
  361 blanks, `input_tokens` p50 **22**, and **323** assistant replies of
  literally `"Waiting."`.

| session | debug | build | status | blanks | reply |
|---|---|---|---|---|---|
| `blossom` | true | self-dev | Crashed | 600 | `"Idle."` |
| `piglet` | **false** | **none** | **Closed** | 361 | `"Waiting."` |
| `tulip` | true | self-dev | Crashed | 194 | |
| `retriever` | false | self-dev | ok | 71 | |
| `rabbit` | false | self-dev | ok | 15 | |

So this is **not** a self-dev tester artifact and not a crash artifact. It
reaches ordinary sessions. The constant 22 input tokens across both means the
provider is repeatedly handed an empty turn, and the model answers with a
one-word acknowledgement because there is nothing to answer.

What drives it is still **unidentified**. Every text-carrying producer is
excluded by output-matching, the reminder path is excluded by token accounting,
and the poke path is now excluded by direct test. The remaining shape is
something that sets a dispatch flag and sends with no body at all — but I have
named three suspects and been wrong three times, so this one gets no name
until a trace shows it.

### One lead, stated as a lead

The loop *onset* correlates with the `todo` tool. Taking the nearest
`tool_use` before the first blank in every session that has one:

| population | nearest preceding tool |
|---|---|
| runaway (>=15 blanks), n=5 | **todo 4**, bash 1 |
| control (1-14 blanks), n=43 | bash 22, todo 5, swarm 4, write 3, bg 3, ... |

`bash` dominates the control population, as it does overall; `todo` dominates
the runaway population. In `blossom` and `piglet` the shape is identical — a
`todo` call, one closing text reply, then the first blank.

`piglet`'s model says so outright on the first blank turn:

```
"Your message came through empty."
```

**This is n=5 and it is a correlation.** It is recorded because it is the
first structural signal about *onset* rather than *content*, and because the
next trace should start there. It is not a cause, and after three wrong
suspects it is not being written up as one.


What *is* established by evidence, and does not depend on the above:

* The loop is driven by an unbounded auto-poke gate on a never-completed todo
  list. Bounding repeats would stop it regardless of where the body is lost.
* The turns are empty on the wire, not merely blank in the transcript
  (22 vs 90 input tokens).
* PR #81 does not address this: it corrects the persisted content, and these
  turns take the promote branch.

## Not inspected

* Whether non-Anthropic providers (OpenAI Responses, Gemini, Copilot) tolerate a
  zero-block user message, or drop it the same way. Only the Anthropic path was
  read. The shipped fix never stores a zero-block message, so this is latent
  rather than active.
* Whether the two producers should converge on one continuation mechanism rather
  than both encoding intent as "empty body + reminder".
* Whether the cache-breakpoint selection in `lib.rs` (keyed on "last assistant
  message" / "second-to-last assistant message") shifts when the trailing
  message role changes.
* Whether `content_bytes=0` on an inbound `message` request should be rejected
  server-side as a defense in depth, independent of the client-side cause. The
  wire log is where this was first visible, so it is also where it could be
  caught unconditionally.

## Method note: two agents, one worktree

While investigating this I read a co-agent's *uncommitted* edit to
`turn_execution.rs`, mistook it for the code on `main`, and told them their
failing test was stale. It was not. Verifying against the committed object
settles it immediately:

```
$ git show f25a1c026:crates/jcode-app-core/src/agent/turn_execution.rs \
    | grep -c CONTINUATION_MARKER
0
```

Two lessons, both cheap to apply:

* **`git status` is not the working tree's author.** With more than one agent
  in a single checkout, uncommitted content on disk may belong to someone
  else. Read `git show <rev>:<path>`, not the file, when the claim is about
  what ships.
* **`git add -A` is unsafe under concurrency.** It swept a peer's in-flight
  source into four docs commits. Nothing was lost, but the real damage was
  epistemic: their half-finished edit became my evidence. Stage explicit
  paths you own.

The same collision struck twice, because the explanatory comment I cited as
"main documenting its own behavior" was also the peer's uncommitted text.

## The loop begins when the work finishes

The onset correlates with the `todo` tool, and the correlation is not a
base-rate artifact. Across 2377 sessions `todo` is only 3.8% of all tool
calls, yet it is the last tool before 4 of the 5 runaway onsets:

```
P(>=4 of 5 | p=0.038) ~ 1e-5
```

The mechanism, though, is the **opposite** of the incomplete-todo poke. At the
exact onset call every todo is already `completed`, and each carries a
`completion_confidence`:

| session | todos at onset | statuses | missing confidence | `needs_more_work` |
|---|---|---|---|---|
| `blossom` | 8 | all `completed` | 0 | false |
| `piglet` | 7 | all `completed` | 0 | false |

That selects the `✅ Todos complete` branch of
`schedule_auto_poke_followup_if_needed`, which disables poking, clears
`pending_queued_dispatch`, returns `false`, and **queues no message**. It is
the quiet, correct branch.

And the agent does stop working. Counting `tool_use` blocks after the first
blank:

```
blossom   15 tool uses in the 1262 messages after onset
piglet     1 tool use  in the  741 messages after onset
```

So the pump does not start when the agent has work left. It starts the moment
the agent declares it has none, and it fires ~600 and ~360 times respectively
against a session with nothing left to do. The remaining question is which
caller of that `false` return keeps dispatching anyway; there are three, in
`server_events.rs`, plus one in `local.rs`. That trace is next, and no fix
should land before it.

## The four callers do not dispatch: a negative result

Stated in advance: *if none of the four callers of
`schedule_auto_poke_followup_if_needed` acts on a `false` return, the driver is
not in this function and the todo-onset signal is a symptom, not the cause.*
That is what the code shows.

| caller | behavior on `false` |
|---|---|
| `local.rs:502` | `clear_visible_turn_started()`, maybe notify. No send. |
| `server_events.rs:1055` | `clear_visible_turn_started()`. No send. |
| `server_events.rs:1169` | `clear_visible_turn_started()`, maybe notify. No send. |
| `server_events.rs:1385` | returns the value. No send. |

The `|| schedule_overnight_poke_followup_if_needed()` fallback that runs on
every `false` is likewise inert here: all five of its early exits return
`false` without queueing, and `overnight_auto_poke.is_none()` in an ordinary
session anyway.

So the quiet "todos complete" branch is not the pump. It is merely the last
thing that happens before the pump starts, which is why it correlates so
sharply. **Correlation confirmed, causation refuted.**

### Where the dispatch actually happens

The event loop, not the scheduler, is what sends:

```rust
// run_shell.rs:374 (local) and :557 (remote)
} else if self.pending_queued_dispatch {
    self.pending_queued_dispatch = false;
    process_queued_messages(...).await;   // or process_remote_followups(...)
}
```

The flag is the sole gate; nothing checks that a message exists. Both
consumers do guard themselves (`process_remote_followups` returns early on
`pending_queued_dispatch`, and the startup path clears
`submit_input_on_startup` and skips on empty input), so no single read of this
code names the offender. Whatever sets `pending_queued_dispatch` without
leaving a message behind is the pump, and finding it needs a live trace of the
flag's transitions, not more reading.

**Status: the driver is still unnamed.** Four suspects have now been refuted
by evidence (reminder, poke, queue desync, and the todo-completion branch).
The next step is instrumenting the flag itself in a live repro.

## Every setter queues something; the empty body is made downstream

All five setters of `pending_queued_dispatch` were checked. None sets the flag
with nothing behind it:

| setter | what it queues first |
|---|---|
| `commands_overnight.rs:411` | `queued_messages.push(prompt)` |
| `commands.rs:2493` | `queued_messages.push(prompt)` |
| `input.rs:1288` (confidence gate) | `hidden_queued_system_messages.push(...)` |
| `input.rs:1307` (incomplete poke) | `queued_messages.push(build_poke_message(...))` |
| `input.rs:1313` | guarded by `has_queued_followups()` |

So my previous framing -- "something sets the flag without leaving a message"
-- was wrong. The queue is never empty at dispatch.

The empty *body* is manufactured one layer down, in `partition_queued_messages`
(`helpers.rs:172`). It splits the queue into user messages and reminders, and
a reminder-only queue yields `user_messages == []`. The caller then does:

```rust
let combined = messages.join("\n\n");                       // "" when messages is empty
let auto_retry = reminder.is_some() && messages.is_empty(); // true in exactly that case
```

`combined` is the user turn. A dispatch carrying only a hidden reminder sends
`""` as the user message by construction. That is a real defect and it is the
shape of the confidence-gate setter, the one setter that queues into
`hidden_queued_system_messages` alone.

### Why this still is not the pump

Stated as the disproof: *if the loop's blanks carried a reminder, this path
explains them; if they carry none, it does not.* The token accounting already
answered. Loop replies cost a flat 22 input tokens, against 90 for a turn that
carried a real reminder in the same session on a warm cache. There is no
reminder in the loop's blanks. The confidence gate also fires only when
`needs_more_work` is true, and at onset it is false in every observed session.

Nor can the resend machinery recycle a bare blank:
`recover_undelivered_queued_continuation` requires
`!pending.content.trim().is_empty() || pending.system_reminder.is_some()`, so
a blank with no reminder is not recoverable by that path.

**Net:** a genuine empty-user-turn generator found and documented, distinct
from all four refuted suspects, but the evidence says it is not what pumped
these five sessions. Worth fixing on its own account (a reminder-only dispatch
should not send `""` as the user body); not the answer to the loop.

Five suspects down. The driver remains unnamed, and the live flag trace is
still the next step.

## The loop is turn-driven, not timer-driven

Before instrumenting anything, two deductions worth stating, because together
they rule out the experiment I was about to run.

**1. The queued-dispatch path cannot produce these blanks.** Chain the
established facts:

* the loop's blanks carry no reminder (flat 22 input tokens vs 90 with one);
* every setter of `pending_queued_dispatch` queues a message or a reminder;
* `combined == ""` arises only from a reminder-*only* queue, which by
  definition has a reminder.

A blank with no reminder therefore cannot come from that path at all. Tracing
`pending_queued_dispatch` transitions would have measured the wrong thing.

**2. It is not a retry timer either.** The rate-limit/network resend at
`remote.rs:196` re-sends `pending.content` verbatim with no emptiness check,
which makes it a plausible pump. Stated disproof: *a timer fires on a fixed
schedule, so timer-driven gaps cluster tightly (CV ~ 0.0-0.2), while
turn-driven gaps track model latency and vary widely.* Measured on the blank
turns' own timestamps:

| session | n | min | median | p75 | max | CV |
|---|---|---|---|---|---|---|
| `blossom` | 596 | 2.4s | 3.6s | 4.5s | 286s | **3.66** |
| `piglet` | 360 | 2.0s | 3.3s | 3.8s | 269s | **3.96** |

CV near 4 is nowhere near a timer. The ~3.5s median is exactly a short model
round trip, and the long tail is ordinary provider variance. So each blank is
sent, answered, and the answer triggers the next one: a **send -> reply ->
send** cycle running at model speed, not a scheduler firing into the void.

That reframes the search. The pump is not something that keeps *setting a
flag*; it is something in the turn-completion path that treats a completed
turn as grounds for starting another, with no user input and nothing queued.
`finish_turn` and the `Done` handler are where that decision is made, and both
already appear in this document as the callers that were inert on `false`.
The remaining candidate is therefore a send site that does not route through
`pending_queued_dispatch` at all: there are 20 callers of `begin_remote_send`
outside the queue path (7 in `remote.rs`, 10 in `key_handling.rs`, 2 in
`input_dispatch.rs`, 1 in `tui_lifecycle.rs`).

Six suspects refuted. The driver is still unnamed, but the search space is now
a specific list rather than a whole subsystem.

## Correction: the empty join is real, my trigger for it was not

@badger checked the mechanism and found the conclusion right but the path
wrong. Verified here before accepting:

```
remote.rs:215    if !app.is_processing && !app.queued_messages.is_empty() {
remote.rs:1351   } else if !app.queued_messages.is_empty() {
```

Both joins gate on `queued_messages` being non-empty. The confidence gate I
named (`input.rs:1288`) pushes only to `hidden_queued_system_messages` and
never touches `queued_messages`, so neither branch can run. It also sets
`pending_queued_dispatch`, which trips the early return at `remote.rs:211`.
**The confidence gate cannot produce the empty join. Retracted.**

The reachable path is a queue that is non-empty but *entirely bracketed*:
`extract_bracketed_system_message` routes every `[SYSTEM: ...]` entry into
`reminder_parts`, leaving `user_messages` empty. Non-empty in, empty out.
Badger's test covers it:

```
partition_queued_messages_yields_no_user_text_when_queue_is_all_system ... ok
```

So the second defect stands, with a corrected trigger.

### It is still not the pump

Stated disproof: *the producers that queue only bracketed messages
(`queue_startup_message`, `set_ambient_mode`) are one-shot at session start;
if the loop were theirs, onset would sit at the beginning of the session.*

| session | first blank | of | position |
|---|---|---|---|
| `blossom` | msg 449 | 1711 | 26% in |
| `piglet` | msg 161 | 902 | 18% in |

Onset is deep mid-session, after hundreds of healthy turns, and then repeats
hundreds of times. A one-shot startup producer cannot do that. Seven suspects
refuted.

### What the loop's replies rule out

The assistant reply that precedes each next blank is almost always pure text:

| session | `text` only | with `tool_use` |
|---|---|---|
| `blossom` | 591 / 597 | 4 |
| `piglet` | 357 / 361 | 1 |

No tool_use means no tool-result continuation is driving the cycle, and
`stop_reason` is `None` on every one of them. So the next send is triggered by
an ordinary completed text turn, which is what makes this a **send -> reply ->
send** loop at model speed rather than any resume or continuation mechanism.

## A near-miss worth recording: process identity

The `Done` handler's return value was worth re-checking, since the loop being
turn-driven seemed to contradict my reading that the handler is inert. It does
not: `run_shell.rs:583` consumes the boolean in a `match` over
`RemoteEventOutcome` whose only arms are `Continue`/`Reconnect`/`Quit`, and the
value otherwise feeds `needs_redraw`. It cannot start a turn. The reading
stands, so the sender is elsewhere.

That prompted asking the sessions who sent the turns rather than asking the
code. `piglet` was *created* by pid 2203 and its `last_pid` is 52888. A
different process finished the session than started it, which fits a
send -> reply -> send loop driven by something that reattached.

The fleet seemed to confirm it emphatically:

| population | n | create-pid != last-pid |
|---|---|---|
| runaway (>=50 blanks) | 4 | **100%** |
| all controls | 2127 | 10% |

**Then the confound check killed it.** Long sessions get reattached far more
often, and every runaway session is long (>= 850 messages). Comparing against
*length-matched* controls instead:

| population | n | create-pid != last-pid |
|---|---|---|
| runaway | 4 | 100% |
| controls with >= 850 messages | 28 | **82%** |

100% against 82% at n=4 is nothing at all. The apparent 10x signal was
entirely session length. Recorded because the unmatched comparison looked
decisive and would have sent the investigation into reattach handling on the
strength of an artifact.

Method note: the base-rate check that promoted the todo-onset signal (3.8% vs
4-of-5) and the confound check that demoted this one are the same test. Any
population comparison here needs a control matched on session length, since
length drives blanks, reattaches, tool counts, and nearly everything else.

## The wire says it plainly: `content_bytes=0`, and the queue was always empty

Every section above reasons about the client from the *session file*, which is
the output of the thing under investigation. The server log is an independent
record of the same events, and it had been sitting in `~/.jcode/logs/` the
whole time. Reading it took ten minutes and settled in one command what eight
suspects could not.

`piglet` (2026-07-30) is the only runaway session inside the log retention
window that also postdates `SERVER_REQUEST_LIFECYCLE` logging. `blossom` and
`tulip` are 07-29; that log exists but has **zero** `phase=received` lines, so
the wire-level check cannot be run against them. Everything below is one
session, and is labelled as such.

### The pump has a fingerprint

```
$ grep session_piglet ... | grep phase=received | grep -c 'content_bytes=0'
361
$ grep session_piglet ... | grep phase=received | grep -vc 'content_bytes=0 '
5
```

361 empty requests, exactly matching the 361 blank user turns in the session
file. `content_bytes` is the literal `content` field of the decoded wire
request (`request_payload_summary`, `client_lifecycle_logging.rs:68`), so this
is not an inference about what the client meant to send. It is what it sent.

Ordered by request id, the session's whole life is visible:

| request_id | kind | content_bytes |
|---|---|---|
| 3 | message | 6128 |
| 4 | soft_interrupt | 155 |
| 5 | soft_interrupt | 85 |
| **6 .. 365** | **message** | **0** |
| 366 | cancel | (27 byte envelope) |
| 367 | message | 0 |

The pump starts at id=6 and never recovers. It ran for **63 minutes**
(16:44:18 to 17:47:17) and stopped only because the user pressed Escape:
request 366 is `REMOTE_INTERRUPT_SEND_START kind=cancel
trigger=keyboard_escape`. That is the only keyboard-originated event in the
entire hour.

### Onset, to the millisecond

```
16:44:18.783  API call complete in 19.51s (input=1293 output=1135)
16:44:18.809  Turn complete - no tool calls
16:44:18.834  Client received Done id=3, current_message_id=Some(3)
16:44:18.845  SERVER_REQUEST_LIFECYCLE phase=received request_kind=message content_bytes=0
```

**11 milliseconds** from the client observing `Done` to the client sending an
empty message. This confirms the CV-based inference from the timing section
above by direct observation: the loop is `Done` -> send -> `Done` -> send, at
model round-trip speed.

### This kills the entire queue path, including my own leading suspect

Two independent facts from the log, either of which is sufficient:

1. **The queue was never non-empty.** The client emits its full state in
   `TUI_SLOW_FRAME`. Across all 96 frames spanning the session:

   ```
   queued_messages: Counter({0: 96})
   ```

2. **No reminder ever rode along.** None of the 361 empty requests carries a
   `system_reminder` field, and `line_bytes=251` is pure envelope.

Every remaining candidate in `process_remote_followups` requires one or the
other. The two queue joins gate on `!queued_messages.is_empty()`; both
hidden-reminder branches pass `Some(combined)` and would show a reminder and
cost ~90 tokens rather than 22. The fallback resend logs `"Resending failed
turn"`, which never appears. `partition_queued_messages` returning an empty
user set - the defect I found, handed to badger, and quietly hoped was also the
pump - **cannot** be it, because it requires a non-empty queue to partition.

That was my best remaining lead and the data killed it outright.

### Where the seed is

Two soft interrupts were sent while a turn was running (ids 4 and 5). Both were
injected and committed server-side (`AGENT_SOFT_INTERRUPT_INJECT_COMMIT` x2).
But the *client* counter never returned to zero:

```
16:23:00.056  kind=soft_interrupt_injected content_bytes=155 pending_soft_interrupts=1
16:25:19.493  kind=soft_interrupt_injected content_bytes=85  pending_soft_interrupts=1
```

After the second injection the client still believes one interrupt is pending,
and it stays that way for the rest of the session:
`recover_stranded_soft_interrupts` logs on every run and appears **zero** times
in the log, as does its dedup line and its failure line. So a non-empty
`pending_soft_interrupts` sat in client state, unrecovered, from 16:25 until
the session closed - 19 minutes before onset, and through all 361 sends.

Three further constraints from the same log:

- Every frame is `input_event: tick`. No keystroke is involved.
- One client instance, one connection (1096 and 1101 lines respectively). This
  is not the multi-client contention that
  `recover_stranded_soft_interrupts` was written to defend against.
- All 361 sends funnel through `begin_remote_send`, since
  `send_message_with_images_and_reminder` has exactly one non-test caller
  (`input_dispatch.rs:19`), and `begin_remote_send` has **no** emptiness guard.

### What is still not proven

The precise tick branch that calls `begin_remote_send` with `content: ""` while
`pending_soft_interrupts` is stuck is **not yet identified**, and the obvious
candidate is already dead: `retrieve_pending_message_for_edit` is the one
function that drains that vector into the input box, and all three of its
callers are keystroke paths (Up-arrow, Escape) while every observed frame is a
tick.

Two rounds of reading the code for a branch that fits have now failed. The next
step is instrumentation, not more reading: log content length and the relevant
state at the single wire chokepoint and reproduce with a deliberately stranded
`pending_soft_interrupts`.

Stated so it can be checked rather than believed: the stuck counter is
correlational. It is the only anomalous client state that persists across the
onset, and it appears 19 minutes before the first blank, but nothing here
demonstrates that it *causes* the send.

### Method note

The evidence that settled this was on disk before the investigation started.
Eight suspects were refuted by reasoning about the session file, which is the
artifact the bug produces; the server log is an independent observation of the
same events and answered the question directly. When an investigation stalls,
check whether an independent record of the same events exists before designing
another experiment against the dependent one.

## All twenty call sites are refuted, which means an exclusion is wrong

`begin_remote_send` is the only client function that reaches the wire with a
`Request::Message`, and it has twenty non-test call sites: `key_handling.rs`
10, `remote.rs` 7, `input_dispatch.rs` 2, `tui_lifecycle.rs` 1. Making that a
number rather than a narrative was the useful move, because each site predicts
an artifact that either exists or does not.

| call site | prediction if it is the pump | observed | verdict |
|---|---|---|---|
| `key_handling.rs` (x10) | keyboard `input_event` | 96/96 frames `tick`; sole key event is the closing Escape | dead |
| `remote.rs:196` rate-limit resend | ~361 "Retrying continuation" / "Rate limit reset" system messages | **0** in `.json`, `.bak`, and `.evidence.jsonl` | dead |
| `remote.rs:232` queue join | non-empty `queued_messages` | 0 in 96/96 frames (41 pre-onset, 55 post-onset) | dead |
| `remote.rs:269` hidden reminder | `system_reminder` on the wire | 0/361 | dead |
| `remote.rs:1156` fallback resend | `"Resending failed turn"` | 0 lines | dead |
| `remote.rs:1343` interleave | non-empty guard, plus `interleave` in log | 0 lines, and guarded | dead |
| `remote.rs:1374/1402` | queue/reminder state | same as above | dead |
| `input_dispatch.rs:132/310` submit | keystroke | tick-only | dead |
| `tui_lifecycle.rs:90` wrapper | delegates to `remote.rs` | n/a | dead |
| deferred pre-history prompt | `"Dispatching prompt that was held"` | **1** line, cannot explain 361 | dead |
| synthetic startup dispatch | `"Dispatching restored startup/queued followup"` | 0 lines | dead |

Twenty sites refuted against 361 observed sends is a contradiction, not a
result. One of the exclusions above is false, and the honest position is that
the pump is not yet named.

Two exclusions were re-tested rather than trusted, since both were mine:

- **The queue claim survived.** `TUI_SLOW_FRAME` only fires on *slow* frames,
  so "0 in 96/96" could have been a sampling artifact covering only the quiet
  period. Parsing the frames as JSON and splitting on the onset timestamp
  gives 41 pre-onset and 55 post-onset frames, all with
  `"queued_messages":0` and all `"input_event":"tick"`. The claim holds inside
  the pump window specifically.
- **One earlier refutation was invalid.** Grepping the server log for
  `"Rate limit reset"` returned 0 and I read that as exclusion, but that string
  is a `DisplayMessage`, never a log line, so the grep tested nothing. The
  correct test is against the session file, where the banner would have been
  persisted 361 times. It appears 0 times, so the conclusion stands, but it
  stood on a broken test for one iteration.

`client_api.rs:49` constructs `Request::Message` outside the TUI entirely and
was never in the candidate set. It is excluded by evidence rather than by
reading: it opens its own connection with its own id counter starting at 1,
while piglet's empty sends are contiguous ids 6..367 on the *same* connection
that carried the real request id=3.

One incidental finding: `TURN_CANCEL_REGISTERED` appears 362 times, once per
empty send, always `active_turns=1`, always paired with an unregister. That is
not a leak. It confirms each empty request became a full server-side turn.

### Where this stops

Three rounds of static reading have now failed to name the branch, and the
third produced a contradiction instead of a candidate. Continuing to read is
the wrong move. The next step is instrumentation at the single chokepoint,
recording caller identity and the state that gates it, because the pump's
distinguishing property is that it logs *nothing at all* -- and every branch
that logs has been eliminated by that silence.

## Named: the todo completion-confidence gate, proven byte-exact

The contradiction resolved the way contradictions usually do: one exclusion was
invalid, and it was mine.

I had written "no `system_reminder` field on any empty request (0/361)" and
used it to kill every reminder-bearing branch. **That test never ran.**
`request_payload_summary` (`client_lifecycle_logging.rs:68`) only extracts
`content`, `message`, `prompt`, `task`, `command`, `input`, `value`, plus
`images`/session ids. `system_reminder` is not in the list, so it is never
logged, and its absence from the log means nothing at all. This is the same
error class as grepping the server log for the `"Rate limit reset"`
`DisplayMessage`: **an exclusion resting on a missing string, where the string
was never emitted on that path.** Twice in one investigation.

What is real is `line_bytes`, because the server logs `line.len()` directly.
And it was visible the whole time that the empty envelope is *too big*:

| request | content_bytes | line_bytes | overhead |
|---|---|---|---|
| id=3 (real user turn) | 6128 | 6291 | **163** |
| id=6 (empty) | 0 | 251 | **251** |

An empty message carries ~88 bytes *more* envelope than a full one. Since
`system_reminder` is `#[serde(skip_serializing_if = "Option::is_none")]`
(`wire.rs:129`), a fatter envelope with no content means the field is present.
Solving for it gives a reminder of ~191 characters.

`TODO_COMPLETION_CONTINUATION_MESSAGE` (`crates/jcode-base/src/todo.rs:27`) is
**exactly 191 characters**. Predicting `line_bytes` from the real wire struct,
with `+1` because `read_line` retains the newline and `images` is skipped when
empty:

| request ids | predicted | observed | count |
|---|---|---|---|
| 6-9 | 251 | 251 | 4 / 4 |
| 10-99 | 252 | 252 | 90 / 90 |
| 100-367 | 253 | 253 | 267 / 268 |

Byte-exact across all three id-width buckets. The single-request shortfall in
the last bucket is id=366, the user's Escape (`request_kind=cancel`). A scan of
every string literal in the crate tree found 11 within +/-2 characters of 191,
and exactly one is a `system_reminder`; the rest are format strings, test
fixtures, and unrelated notices. `hidden_queued_system_messages` has only two
originating `push` sites (the others are re-queue-on-failure paths that recycle
an existing reminder), and the other one is reload-recovery with a different
message and a dedup guard.

**The pump is `schedule_auto_poke_followup_if_needed`, `input.rs:1281-1289`:**

```rust
if confidence_summary.needs_more_work {
    self.push_display_message(DisplayMessage::system("🛑 Todo completion gate: ..."));
    self.hidden_queued_system_messages.push(build_todo_confidence_summary_message(&todos));
    self.pending_queued_dispatch = true;
    return true;
}
```

Returning `true` means "followup scheduled", so the `Done` handler skips
`clear_visible_turn_started()`; the queued reminder is then dispatched by the
`remote.rs:269` hidden-reminder branch as `String::new()` content plus
`Some(reminder)`. The model answers, the turn completes, and the gate is
re-evaluated against **the same unchanged todo list** - so it fires again,
forever, at model round-trip speed. Nothing drains it because nothing about
answering a reminder changes `completion_confidence`.

This also explains every constraint that made the bug hard to find:

- **Why it logs nothing.** It does log (`"Sending hidden continuation
  reminder"`), but that is `crate::logging::info`, and the 07-30 run was not at
  info level for the client. The 5 lines found fleet-wide are from runs that
  were.
- **Why the queue was empty.** `hidden_queued_system_messages` is a *separate*
  field from `queued_messages`, and only the latter appears in `TUI_SLOW_FRAME`.
  My "0 in 96/96 frames" was correct and irrelevant.
- **Why 22 input tokens.** Confirmed earlier as `""` plus a timestamp. The
  reminder rides on the system prompt, not the user message.
- **Why it starts when work finishes.** The gate is only reachable when
  `incomplete.is_empty()` - every todo marked completed - which is exactly
  badger's finding that all 598 onsets follow a *text-only* assistant turn and
  never one ending in `tool_use`.
- **Why the todo correlation was p~1e-5.** It was not incidental after all. The
  gate reads the todo list directly.

Two agents converged from opposite ends: badger's timing split showed the
dispatch leg was local (162ms) and every onset followed a completed turn, while
the envelope arithmetic named the payload. Neither alone was sufficient.

### The fix, and why the guard is not it

The `String::new()` skip I had staged in `remote.rs` would have suppressed the
symptom and left the gate looping invisibly. The defect is that
`needs_more_work` is re-evaluated against state the reminder cannot change, so
the gate must fire **at most once per todo-list revision**. That is the change
to make, and it belongs with a test that asserts a second identical evaluation
does not re-queue.
