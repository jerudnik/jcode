---
title: "OpenAI HTTPS transport re-issues the request after forwarding a terminal API error"
status: open
priority: high
owner: unassigned
opened: 2026-09-21
related:
  - crates/jcode-provider-openai-runtime/src/openai_stream_runtime.rs
  - crates/jcode-provider-openai-runtime/src/openai_tests/persistent_continuation.rs
---

# OpenAI HTTPS transport re-issues the request after forwarding a terminal API error

Commit `d7eafdb18` taught both WebSocket paths to forward a non-retryable
API error once and return `TerminalError` so the caller does not replay.
The HTTPS/SSE path was not changed and still has the original asymmetry.

In the HTTPS branch of `stream_response` (`openai_stream_runtime.rs`, around lines
285-309 and 359 at `d7eafdb18`):

1. A `StreamEvent::Error` whose message is not retryable is forwarded to the
   consumer and streaming continues.
2. The server then closes the SSE stream without a `MessageEnd`.
3. The "stream ended before message completion marker" branch returns
   `OpenAIStreamFailure::Other`, which the caller classifies as retryable,
   so the same request is issued again.

The consumer usually sees a single error because the agent loop aborts on
the first one, but a duplicate billed request can still go out, and any
tool call that was partially streamed before the error is re-requested.

This was found by the independent verifier of `d7eafdb18`, reasoned from
the code. It has not been reproduced: the loopback fixture in
`persistent_continuation.rs` is WebSocket-only. A fix should first extend
the fixture (or add an SSE one) so the failure is observed before the
`TerminalError` handling is mirrored onto the HTTPS path.

## Also noted by the same review, lower priority

- `PersistentWsState::last_input` now retains the full canonical input
  `Vec<Value>` per turn. Per-item `stable_hash_json` fingerprints would give
  the same exact-prefix guarantee at a fraction of the memory.
- The two terminal-failure tests detect the prior behavior as a 3 s timeout,
  which is timing-sensitive under heavy load.
