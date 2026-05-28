---
name: unattended-execution
description: Use for attended or unattended workflows that need reliable progress, deterministic state checks, failure triage, validation gates, cache-aware retries, and completion evidence.
allowed-tools: bash, read, write, edit, multiedit, apply_patch, agentgrep, batch, bg, todo, schedule, selfdev
---

# Unattended Execution

Use when a task may run without user supervision or needs a high-confidence attended workflow.

## Default gate

1. Snapshot state: `git status --short --branch`, relevant files, active background tasks, and current task/todo state.
2. Define evidence: exact commands, files, endpoints, cache hits, rendered artifacts, or tests that prove completion.
3. Run targeted validation before broad validation.
4. Commit focused changes, push, then verify clean/synced state.
5. Report evidence, residual risks, and any scheduled follow-up.

## Long-running commands

- Prefer background tasks for builds, deploys, crawls, and long test suites.
- Emit progress from custom scripts:

```bash
printf 'JCODE_PROGRESS {"current":1,"total":3,"unit":"steps","message":"building"}\n'
printf 'JCODE_CHECKPOINT {"message":"targeted tests passed"}\n'
```

- Never fire-and-forget: wait for completion or schedule a resume check.
- Use bounded timeouts. If a command can hang, wrap the inner command with `timeout` too.

## Failure triage

Classify failures before retrying:

- Code or test bug: reproduce narrowly, fix, rerun.
- Flaky infra/network/cache: retry once with the same bounded command, then record as blocker.
- Missing secret/auth: do not print secrets; report the missing capability or token path.
- Baseline unrelated failure: capture command/output and keep it separate from task result.
- Destructive risk: stop before irreversible mutation.

## Deterministic state checks

Prefer checks that can be rerun and compared:

- `git diff`, `git status --short --branch`, `git log -1 --oneline`
- test/build exit codes and saved logs
- file existence, hashes, line counts, generated artifacts
- service health endpoints, process status, sockets, cache `.narinfo` probes
- rendered screenshots or snapshots for UI changes

## Cache-aware builds

When builds should warm SMFS Attic:

1. Confirm substituter: `nix show-config | rg 'substituters|trusted-public-keys'`.
2. Build once on a host with the Attic push hook/token.
3. Verify cache presence with the store hash `.narinfo` at the Attic substituter.
4. Retry the consumer command and confirm it substitutes instead of rebuilding.

## Completion confidence

Only mark done when evidence matches the claim. If confidence is low, schedule or record follow-up instead of declaring success.
