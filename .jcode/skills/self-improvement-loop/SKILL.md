---
name: self-improvement-loop
description: Use for implementation, debugging, hygiene, research, or planning tasks where the agent should iteratively inspect, reproduce or measure, implement, self-evaluate, research, validate, and record/commit results.
allowed-tools: bash, read, write, edit, multiedit, apply_patch, grep, agentgrep, batch, todo, selfdev
---

# Self-Improvement Loop

Use this skill when taking a task from intent to a verified, recorded outcome.

## Core loop

1. **Inspect**
   - Read the task, acceptance criteria, references, docs, and related code/state.
   - Identify the invariants that must hold and any boundaries that must not be crossed.
   - Check the current repo/task state before changing anything.

2. **Reproduce or measure**
   - Prefer a focused failing test, validation command, inventory, metric, or minimal reproduction before changing behavior.
   - If reproduction is not applicable, define objective evidence for completion.
   - Capture unrelated baseline failures separately instead of hiding them.

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

6. **Validate broadly enough**
   - Run targeted validation first.
   - Then run relevant broader tests, checks, or builds.
   - For jcode code changes, prefer `selfdev build` before completion and reload after a clean committed build when appropriate.

7. **Record and close**
   - Update Backlog notes, acceptance criteria, Definition of Done, modified files, and final summary as applicable.
   - Commit and push focused changes.
   - Confirm clean git state and no unintended active tasks.

## Backlog plan template

```text
Self-improvement loop:
1. Inspect task context and related code/docs.
2. Reproduce or measure the current failure/invariant.
3. Implement the smallest maintainable fix.
4. Critically review completeness and edge cases.
5. Self-evaluate, research with agent-toolbox when useful, check deterministic state, and steer the next pass.
6. Run targeted and relevant broader validation.
7. Record final summary, commit, push, and rebuild/reload if needed.
```

## Guardrails

- Do not mark work complete without evidence.
- Do not delete runtime state or secrets as part of hygiene/planning work.
- Do not conflate unrelated validation failures with the current task. Record them clearly.
- Prefer small commits that tell a coherent story.
