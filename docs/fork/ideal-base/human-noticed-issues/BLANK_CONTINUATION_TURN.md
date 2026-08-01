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
