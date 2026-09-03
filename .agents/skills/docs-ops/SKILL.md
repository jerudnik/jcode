---
name: docs-ops
description: Use when creating, moving, or editing documentation in the jcode repo.
allowed-tools: bash, read, write, edit, grep, agentgrep, batch, todo
---

# Documentation operations

- Read `docs/README.md` and the nearest `AGENTS.md` before editing docs.
- Place open issues in `docs/issues/` with the required frontmatter.
- Place proposals, project-management notes, and scratch work in the operator's project notes repository, never in `docs/`.
- Do not create `docs/archive/`, `docs/fork/`, or `docs/proposals/`.
- Do not edit generated `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md` files. Edit `.apm/instructions/` primitives and run `apm compile`.
- After moving files, run `scripts/check_docs_references.py` and `scripts/check_agent_instructions.py`.
- Use the `plain-language` skill for human-facing prose.
