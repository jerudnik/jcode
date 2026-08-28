---
title: "Adopt selected upstream (1jehuang/jcode) fixes identified by the 2026-08 divergence survey"
status: open
priority: medium
owner: unassigned
opened: 2026-08-28
---

# Adopt selected upstream (1jehuang/jcode) fixes identified by the 2026-08 divergence survey

A read-only survey of `fork-point..upstream/master` (5,642 post-fork commits,
1,300 first-parent, vs 1,344 on our side) identified a short list of upstream
changes worth adopting. Full evidence, uncertainty notes, and the explicit
skip list live in the operator's Serena memory `upstream/divergence-survey-2026-08`
until this issue is worked; the list below is self-contained either way.

Upstream is MIT-licensed shared ancestry; direct imports are ordinary local
changes and must pass this repository's current gates (repository contract).
Re-check each candidate against then-current `main` for semantic duplication
before importing, and import with provenance (upstream sha in the commit
message).

## Adopt directly (verified clean `git apply --check` at survey time)

1. `250c71acd` — classify `stream_read_error` as transient so the turn retries
   instead of failing. Directly relevant: this exact error class killed two
   swarm workers during the 2026-08-27 provider-identity incident
   (docs/issues/swarm-spawn-model-identity-mismatch.md).
2. `9e8d6e13b` — avoid persisting sessions whose content never changed.
   Complements our orphan-session cleanup and session-scoped cache work
   (PR #209 lineage).
3. `2eaadca31` — restore terminal modes after focus regain (TUI correctness
   after window-manager focus churn).

## Adopt the idea, reimplement to our architecture

4. `c0071abd7` — strict config-update loading (reject unknown/invalid config
   updates instead of best-effort acceptance).
5. `528518ece` — list-style skill `allowed-tools` frontmatter.
6. `6991029b9` — reap detached hook processes.

## Explicitly out of scope (fork policy)

Upstream's operator-only swarm model selection (conflicts with our
catalog-based spawn resolution and routing policy), non-Nix distribution
surfaces, native iOS, upstream-sync machinery, and wholesale architecture
imports.

## Definition of done

Each "adopt directly" item lands as its own commit citing the upstream sha and
passing the full local gate; each "reimplement" item either lands, or is
re-triaged here with a reason. Then delete this issue.
