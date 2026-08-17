---
description: DOX hierarchy and generated instruction ownership contract.
applyTo: "**"
---

# DOX contract

- Repository `AGENTS.md` files are generated work contracts. Before editing, read the root contract and every nearer `AGENTS.md` on the path to each target.
- A nearer contract may add local detail but may not weaken a parent safety or quality rule.
- Edit the owning `.apm/instructions/dox-*.instructions.md` primitive, never a generated `AGENTS.md`.
- Update the nearest primitive when a change alters durable purpose, ownership, structure, workflow, inputs, outputs, constraints, or verification. Do not churn instructions for mechanical edits that leave contracts unchanged.
- Keep one rule at the narrowest durable scope. Remove stale or duplicated guidance instead of explaining its history.
- Validate primitive syntax with `apm compile --validate`; preview placement with `apm compile --dry-run`; regenerate with `apm compile --clean` when files move or disappear.

## Documentation surfaces

- `docs/` holds current docs; `docs/issues/` holds open issues (delete solved). Use `~/notes/projects/jcode/{proposals/,maintenance/,project.md}` for proposals, incident evidence, and PM tracking; keep other records out of `docs/`.

## Child DOX index

- `.apm/AGENTS.md`: APM rules.
- `docs/AGENTS.md`: docs placement rules.
- `docs/issues/AGENTS.md`: open issue rules.
- `crates/jcode-desktop/AGENTS.md`: desktop self-development rules.
