---
name: testing
description: "Use when writing, changing, reviewing, or repairing any test in this repository. Also use when a test fails and you are about to edit the assertion, and when deciding whether an existing test is worth keeping."
allowed-tools: bash, read, edit, grep, agentgrep, batch
---

# Testing contract

Your goal is not to write passing tests. Your goal is to write tests that fail
loudly when the logic under them breaks. A test that cannot go red is worse
than no test: it costs the same to run and it lies.

## Prove it can fail

A test you cannot watch fail is not evidence. Before you keep a new test:

1. Write the test.
2. Break the code it covers, or point it at the unimplemented path, and run it.
   It must fail, and the failure message must name the real cause.
3. Only then write or restore the implementation.
4. Run it again and watch it pass.

If step 2 passes, the test is tautological. Delete it and start over. Say in
your report which assertion you forced to fail and how.

## Banned patterns

Output containing these is rejected:

1. Asserting that a mock returns the value you configured it to return.
2. Assertions inside `try`/`catch` that swallow the error, or inside a callback
   that may never fire. Use the framework's own failure assertion:
   `#[should_panic]`, `assert!(matches!(result, Err(_)))`, `expect_err`.
3. Snapshot tests, unless the request asks for one by name.
4. `assert!(true)`, `assert!(x.is_some())`, or `assert!(result.is_ok())` with
   no check of the payload inside.
5. Asserting on prose. `message.contains("finished their work")` pins wording,
   not behavior. Pin the variant, the field, the status code, the count.

## Assert structure, not text

- Match the exact shape: the enum variant, the type, the field.
- Pin specific field values rather than comparing whole objects.
- Assert that unexpected keys or extra events are absent, not just that the
  ones you want are present.
- Where a string is genuinely the contract (a serialized status, a wire
  format), assert equality against a named constant that production also uses,
  so a rename breaks the build instead of the test.

## Observable behavior only

Assert against public return values and side effects. Reaching into private
state couples the test to the implementation and makes refactoring look like
breakage.

Fixtures must use values production actually produces. A fixture feeding a
status string no code path emits tests nothing. Grep for the value first.

## Strict negatives

For every positive assertion, write the matching negative: the invalid input is
rejected, the unauthorized caller is refused, the terminal state does not
accept the event. A suite with no negative cases has not found any boundary.

## Do not pad

Write the fewest assertions that prove the logic. At most three test cases per
function. Prefer one sharp case over five that cover the same branch. Volume is
not coverage.

## When a test fails

Assume the test found a real defect until you have shown otherwise. Read the
production path before touching the assertion. Editing a test to match new
behavior is a change to the contract and needs the same justification as a code
change: say in the commit why the old expectation was wrong.
