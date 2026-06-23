---
description: DOX rail — the AGENTS.md hierarchy contract and root Child DOX Index for this repo.
applyTo: "**"
---

# DOX framework

- DOX is a hierarchy of `AGENTS.md` work contracts, one per durable directory.
- Agents must follow DOX instructions across any edits.
- In this repo the whole tree is **APM-generated**: each `AGENTS.md` is compiled
  from a `dox-<area>.instructions.md` primitive under `.apm/instructions/`, placed
  by its `applyTo` glob. Edit the primitive, never the emitted `AGENTS.md`.

## Core Contract

- AGENTS.md files are binding work contracts for their subtrees.
- Work products, source materials, instructions, records, assets, and durable docs must stay understandable from the nearest applicable AGENTS.md plus every parent AGENTS.md above it.

## Read Before Editing

1. Read the root AGENTS.md.
2. Identify every file or folder you expect to touch.
3. Walk from the repository root to each target path.
4. Read every AGENTS.md found along each route.
5. If a parent AGENTS.md lists a child AGENTS.md whose scope contains the path, read that child and continue from there.
6. Use the nearest AGENTS.md as the local contract and parent docs for repo-wide rules.
7. If docs conflict, the closer doc controls local work details, but no child doc may weaken DOX.

Do not rely on memory. Re-read the applicable DOX chain in the current session before editing.

## Update After Editing

Every meaningful change requires a DOX pass before the task is done.

Update the closest owning primitive when a change affects:

- purpose, scope, ownership, or responsibilities
- durable structure, contracts, workflows, or operating rules
- required inputs, outputs, permissions, constraints, side effects, or artifacts
- user preferences about behavior, communication, process, organization, or quality
- AGENTS.md creation, deletion, move, rename, or index contents

Update parent docs when parent-level structure, ownership, workflow, or child index changes. Update child docs when parent changes alter local rules. Remove stale or contradictory text immediately. Small edits that do not change behavior or contracts may leave docs unchanged, but the DOX pass still must happen.

## Hierarchy

- Root AGENTS.md is the DOX rail: project-wide instructions, global preferences, durable workflow rules, and the top-level Child DOX Index.
- Child AGENTS.md files own domain-specific instructions and their own Child DOX Index.
- Each parent explains what its direct children cover and what stays owned by the parent.
- The closer a doc is to the work, the more specific and practical it must be.

## Child Doc Shape

Create a child AGENTS.md when a folder becomes a durable boundary with its own purpose, rules, responsibilities, workflow, materials, or quality standards.

Default section order:

- Purpose
- Ownership
- Local Contracts
- Work Guidance
- Verification
- Child DOX Index

## APM mechanism

The DOX tree is generated, not hand-written.

- **One primitive per directory doc.** A child `AGENTS.md` is compiled from a
  `.apm/instructions/dox-<area>.instructions.md` file whose `applyTo` glob targets
  that subtree. This rail (`dox.instructions.md`) is the root contract + index.
- **Placement follows the glob, scored to the deepest directory that directly
  contains matched files.** Preview with `apm compile --dry-run` before relying on
  placement.
- **Add a child doc:** write `.apm/instructions/dox-<area>.instructions.md` with a
  `description`, an `applyTo` glob, and the section shape above; add it to the
  nearest parent's Child DOX Index; run `apm compile`.
- **Remove/rename:** delete or re-scope the primitive, drop it from the parent
  index, recompile. `apm compile --clean` removes orphaned generated files.
- Cross-references in an index use plain backticked paths, not markdown links.

## Style

- Keep docs concise, current, and operational.
- Document stable contracts, not diary entries.
- Put broad rules in parent docs and concrete details in child docs.
- Prefer direct bullets with explicit names.
- Delete stale notes instead of explaining history.

## Closeout

1. Re-check changed paths against the DOX chain.
2. Update nearest owning primitives and any affected parents or children.
3. Refresh every affected Child DOX Index.
4. Remove stale or contradictory text.
5. Run existing verification when relevant.
6. Report any docs intentionally left unchanged and why.

## User Preferences

When the user requests a durable behavior change, record it here or in the relevant child AGENTS.md primitive.

## Child DOX Index

Each entry is a directory `AGENTS.md` generated from the matching
`.apm/instructions/dox-*.instructions.md` primitive.

- `.apm/AGENTS.md` — APM source-of-truth that generates this DOX tree and other agent surfaces.
- `docs/AGENTS.md` — fork maintenance docs, downstream patch ledger, and 4nix integration notes.
- `crates/jcode-desktop/AGENTS.md` — Jcode desktop application self-development context.
