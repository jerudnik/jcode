---
name: self-improvement-loop
description: Use for attended or unattended implementation, debugging, hygiene, research, or planning tasks where the agent should iteratively inspect, reproduce or measure, implement, self-evaluate, research, validate, and record/commit results.
allowed-tools: bash, read, write, edit, multiedit, apply_patch, grep, agentgrep, batch, bg, todo, schedule, selfdev
---

# Self-Improvement Loop

Use this skill when taking a task from intent to a verified, recorded outcome.

## Core loop

1. **Inspect**
   - Read the task, acceptance criteria, references, docs, and related code/state.
   - Identify the invariants that must hold and any boundaries that must not be crossed.
   - Snapshot state before changing anything: `git status --short --branch`, relevant files, active background tasks, and current todo/task state.

2. **Reproduce or measure**
   - Prefer a focused failing test, validation command, inventory, metric, or minimal reproduction before changing behavior.
   - If reproduction is not applicable, define objective evidence for completion.
   - Capture unrelated baseline failures separately instead of hiding them.
   - Define deterministic evidence: exact commands, files, endpoints, cache hits, rendered artifacts, or tests that prove completion.

3. **Implement the smallest high-confidence fix**
   - Make narrow, maintainable changes that address the root cause.
   - Preserve project boundaries and avoid destructive or irreversible actions.
   - Refactor only when it improves correctness, maintainability, or speed of implementation.

4. **Critically review**
   - Re-check whether the change actually addresses the root cause and all acceptance criteria.
   - Look for stale assumptions, adjacent regressions, edge cases, and scope creep.
   - Prefer follow-up tasks over expanding scope silently.

5. **Self-evaluate, research, and steer the next pass**
   - Score the current answer/change against the objective evidence: what is proven, uncertain, missing, or overfit?
   - If uncertainty remains, research deliberately before another implementation pass. Prefer `/agent-toolbox` tools via `nix run $HOME/infrastructure/nix-config#<tool> -- ...` for specialized docs, web, code-intel, and data shaping.
   - Add deterministic state checks: inspect files, diffs, logs, command output, tests, cache hits, process state, or rendered artifacts instead of relying on impressions.
   - Tighten the next prompt/plan/steering instruction from those checks: name the exact hypothesis, evidence target, command, file, or invariant for the next loop.
   - Iterate in the spirit of AutoResearch: generate, check, critique, research, and steer until the result converges or a blocker is explicit.

6. **Run reliably, attended or unattended**
   - For builds, deploys, crawls, and long test suites, prefer background tasks that emit `JCODE_PROGRESS` or `JCODE_CHECKPOINT` lines.
   - Never fire-and-forget: wait for completion, inspect output, or schedule a resume check.
   - Use bounded timeouts. If a command can hang, wrap the inner command with `timeout` too.
   - Before retrying, classify failures as code/test bug, flaky infra/network/cache, missing secret/auth, unrelated baseline failure, or destructive risk.
   - Retry transient infra/cache failures at most once with the same bounded command; otherwise record the blocker.
   - For cache-aware builds, confirm substituters, build on a host with the Attic push hook/token, verify `.narinfo`, then retry the consumer command.

7. **Validate broadly enough**
   - Run targeted validation first.
   - Then run relevant broader tests, checks, or builds.
   - For jcode code changes, prefer `selfdev build` before completion and reload after a clean committed build when appropriate.

8. **Record and close**
   - Update Backlog notes, acceptance criteria, Definition of Done, modified files, and final summary as applicable.
   - Commit and push focused changes.
   - Confirm clean/synced git state, no unintended active tasks, and completion confidence grounded in evidence.

## Backlog plan template

```text
Self-improvement loop:
1. Inspect task context and related code/docs.
2. Reproduce or measure the current failure/invariant.
3. Implement the smallest maintainable fix.
4. Critically review completeness and edge cases.
5. Self-evaluate, research with agent-toolbox when useful, check deterministic state, and steer the next pass.
6. Run long work reliably with progress/checkpoints, bounded timeouts, failure triage, and cache-aware retries.
7. Run targeted and relevant broader validation.
8. Record final summary, commit, push, and rebuild/reload if needed.
```

## Guardrails

- Do not mark work complete without evidence.
- Do not delete runtime state or secrets as part of hygiene/planning work.
- Do not conflate unrelated validation failures with the current task. Record them clearly.
- Do not retry blindly; classify the failure and change the next action based on evidence.
- Prefer small commits that tell a coherent story.
