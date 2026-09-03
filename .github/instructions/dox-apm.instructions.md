---
description: DOX contract for .apm/ — the APM source-of-truth that generates the AGENTS.md tree.
applyTo: ".apm/**"
---

# .apm/ — APM primitives (DOX)

## Purpose

Tracked source-of-truth that APM compiles into every agent-facing output, including this DOX tree.

## Ownership

- `instructions/*.instructions.md` — instruction primitives. `dox.instructions.md` is the global DOX rail; each scoped primitive carries an `applyTo` glob, and APM's optimizer decides whether it becomes that subtree's `AGENTS.md` or folds into the root. Preview with `apm compile --dry-run`.
- `skills/<name>/SKILL.md` — repo-specific skills.

## Local Contracts

- Generated `AGENTS.md`/`CLAUDE.md`/`GEMINI.md` are never hand-edited. To change a directory's instructions, edit the owning primitive and recompile.
- After editing primitives run `apm compile`; after editing `apm.yml` or dependency declarations run `apm install`.
- Generated outputs are committed. Regenerate them in the same commit as the primitive change; `apm audit` and `scripts/check_agent_instructions.py` fail on drift.
- Tool installers must not own generated agent surfaces. Import durable content into `.apm/`, then regenerate.

## Work Guidance

The command loop (validate, dry-run, compile, install, frozen install, audit) and the lock policy live in `docs/agent-workflows.md` under "Agent instruction maintenance". No skill owns APM maintenance.

## Verification

Run `apm compile --validate` and, when placement changes, `apm compile --dry-run`.
