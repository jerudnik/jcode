# Empty user turns reach the provider because hidden continuations persist a blank text block

Reported by human (2026-08-01): messages appearing in the transcript as if the
user had sent nothing, each burning a full model call. Observed 10 times in one
session over ~10 hours.

**Fixed** in `a5fc81b7b` ("fix(agent): stop sending blank user turns on hidden
continuations"). This document records the diagnosis, and in particular records
a *wrong first fix that was merged*, because the failure mode of that fix is the
durable lesson.

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
content is exactly one empty text block, and by that definition there are 10.

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
