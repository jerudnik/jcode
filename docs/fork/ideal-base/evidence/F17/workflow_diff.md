# F17 workflow diff — what became blocking

Diff range: `origin/main..a5f3bf6a8` on `.github/workflows/fork-ci.yml`.
Full patch: `workflow_diff.patch` (same range).

## Semantic changes (advisory -> blocking)

1. **macOS `Build & Test (macOS)` job**
   - `Run workspace library tests`: dropped `continue-on-error: true`.
   - `Run jcode-app-core library tests`: dropped `continue-on-error: true`.
   - **Added** `Run jcode-tui library tests` (executes `-p jcode-tui --lib`;
     previously jcode-tui was compile-only via `--no-run`).
   - `Run e2e tests`: added `JCODE_E2E_REQUIRE_BINARY=1` + `JCODE_E2E_BINARY`
     (the REQUIRE_BINARY export handed over from F16), so e2e fails loudly
     instead of silently skipping when the binary is absent.

2. **`Linux Tests` job**
   - Dropped job-level `continue-on-error: ${{ github.event_name != 'schedule' }}`.
     Linux tests are now blocking on push/PR, not only on the weekly schedule.
   - **Added** `Run jcode-tui library tests` (executes, not compile-only).
   - **Added** `Build jcode binary for e2e` + `JCODE_E2E_REQUIRE_BINARY=1`.

3. **Header + job names** updated to describe the new tiering
   (blocking on push/PR; scheduled runs repeat the blocking Linux coverage).

## What did NOT change
- `Latest stable canary (advisory)` remains `continue-on-error: true` by design.
- Trigger set (push to main, PR to main, weekly cron, workflow_dispatch)
  is unchanged except comment wording.
- No `.github/workflows` diff lands on `main` itself (fork-health invariant);
  these rails ride the `ci-validation` branch / PR #16 until promoted.
