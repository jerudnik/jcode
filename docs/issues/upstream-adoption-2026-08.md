---
title: "Review optional upstream candidates identified by the 2026-08 divergence survey"
status: open
priority: medium
owner: unassigned
opened: 2026-08-28
---

# Review optional upstream candidates identified by the 2026-08 divergence survey

A read-only survey of `fork-point..upstream/master` (5,642 post-fork commits,
1,300 first-parent, vs 1,344 on our side) identified a short list of changes
that may be useful here. This issue is a later-run review queue, not an upstream
sync plan or a commitment to import any item. It is outside the current issue
burndown and remains open for separately scheduled work.

Treat the upstream repository only as optional reference material. Re-check each
candidate against current `main` for need, fit, and semantic duplication. Any
import is an ordinary local change and must pass this repository's current
gates. Cite the upstream SHA in the commit message when code is imported.

## Candidates for direct-import review

These patches passed `git apply --check` at survey time. That result does not
decide whether Jcode should import them.

1. **Adopted in `79cef8482`:** `250c71acd` classifies `stream_read_error` as
   transient so the turn retries instead of failing. Regression coverage ports
   `437c6610a`. This exact error class killed two swarm workers during the
   2026-08-27 provider-identity incident
   (docs/issues/swarm-spawn-model-identity-mismatch.md).
2. `9e8d6e13b` — avoid persisting sessions whose content never changed.
   Complements our orphan-session cleanup and session-scoped cache work
   (PR #209 lineage).
3. `2eaadca31` — restore terminal modes after focus regain (TUI correctness
   after window-manager focus churn).

## Candidate ideas for local implementation review

4. `c0071abd7` — strict config-update loading (reject unknown/invalid config
   updates instead of best-effort acceptance).
5. `528518ece` — list-style skill `allowed-tools` frontmatter.
6. `6991029b9` — reap detached hook processes.

## Not candidates under fork policy

Upstream's operator-only swarm model selection (conflicts with our
catalog-based spawn resolution and routing policy), non-Nix distribution
surfaces, native iOS, upstream-sync machinery, and wholesale architecture
imports.

## Definition of done

Review each candidate against current `main`. For each item, either remove it
with a recorded rejection reason or implement it as a local change with focused
tests and the current repository gates. Delete this issue when no candidates
remain.
