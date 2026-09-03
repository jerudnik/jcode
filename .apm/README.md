# .apm

APM source tree. `instructions/*.instructions.md` are instruction primitives
and `skills/*/SKILL.md` are repository skills; `apm.yml` at the repository
root names the targets. Regenerate every agent surface with `apm compile` and
`apm install`; the loop is in `docs/agent-workflows.md`.

This file also anchors placement. APM nests `.apm/AGENTS.md` only when
`.apm/` itself contains a file; without one, the `.apm/**` rule folds into the
root `AGENTS.md` and breaks the root prompt budget.
