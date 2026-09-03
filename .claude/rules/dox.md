---
paths:
  - "**"
---

# DOX contract

- Repository `AGENTS.md` files are generated work contracts. Before editing, read the root contract and every nearer `AGENTS.md` on the path to each target.
- A nearer contract may add local detail but may not weaken a parent safety or quality rule.
- Edit the owning `.apm/instructions/dox-*.instructions.md` primitive, never a generated `AGENTS.md`.
- Update the nearest primitive when a change alters durable purpose, ownership, structure, workflow, inputs, outputs, constraints, or verification. Do not churn instructions for mechanical edits that leave contracts unchanged.
- Keep one rule at the narrowest durable scope. Remove stale or duplicated guidance instead of explaining its history.
- Validate primitive syntax with `apm compile --validate`; preview placement with `apm compile --dry-run`; regenerate with `apm compile --clean` when files move or disappear.

## Documentation surfaces

- `docs/` holds current docs; `docs/issues/` holds open issues (delete solved). Use the operator's project notes repository for proposals, incident evidence, and PM tracking; keep other records out of `docs/`.

## Nested contracts

- A nested `AGENTS.md` exists wherever APM places a scoped primitive; the set is not enumerated here. `apm compile --dry-run` prints the current placement.
