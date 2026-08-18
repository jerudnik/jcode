---
title: A flaky test asserts on an error message and never prints the error it got
status: open
priority: low
owner: maintainers
opened: 2026-08-18
---

# `test_batch_rejects_function_namespaced_batch_recursion` fails undiagnosably

Verified (2026-08-18): the Linux `Rust checks` leg of PR #166 failed on a single
test in `crates/jcode-app-core/src/tool/tests.rs`. The pull request touches only
comm/protocol files; it does not touch `tool/batch.rs`, `tool/mod.rs`, or that
test. Re-running the same job against the same commit, with no code change,
passed. So the failure is order- or timing-dependent, not a regression.

## What the log could and could not tell us

The whole failure, as it appears in CI:

```text
thread '...test_batch_rejects_function_namespaced_batch_recursion' panicked at ...
assertion failed: error.to_string().contains("Cannot batch the 'batch' tool")
test result: FAILED. 1189 passed; 1 failed; 3 ignored
```

The test calls `registry.execute("batch", ...)` and `expect_err`, so an error
*was* returned -- `expect_err` succeeded. Only the message did not match. The
useful fact is therefore *which* error came back instead, and that is exactly
the fact the assertion discards.

`Registry::execute` can return early for reasons that have nothing to do with
batch recursion: a session tool policy marking the tool disallowed or disabled,
an unknown-tool path with name suggestions, or a `pre_tool` hook returning
`Block`. Each produces a different message, and each would satisfy `expect_err`
while failing this `assert!`. Without the text, all of them look identical from
the log.

## Why order-dependence is plausible here

A sibling test in the same file carries an explicit read-lease comment recording
that tests in this binary mutate process-global environment state via
`crate::env::set_var`, and that spawning reads that state. Tests in one binary
run in parallel by default, and the Linux runner's core count differs from the
Darwin machines the same suite is usually run on, so interleavings differ. That
is enough to explain a rerun-green failure without any code changing.

This entry does not claim to know which sibling caused it. That is the point:
the evidence needed to know was available at the moment of failure and was not
recorded.

## What changed

The assertion now carries the error:

```rust
assert!(
    error.to_string().contains("Cannot batch the 'batch' tool"),
    "expected batch recursion to be rejected, got: {error}"
);
```

This does not fix the flake. It converts the next occurrence from a dead end
into a diagnosis, which is the prerequisite for fixing it.

## Suggested direction

When the next failure names the actual error, the fix follows from which one it
is: a policy or hook leaking across tests points at per-test isolation of the
process-global registry; an unknown-tool message points at registration order.
Until then, adding isolation blindly would be guessing at a cause we have not
observed.

## Related

The defect class is an absent signal read as an adequate one: a bare `assert!`
on a `contains` reports that something was wrong while withholding what, and a
green rerun then makes the question un-askable until it recurs.
