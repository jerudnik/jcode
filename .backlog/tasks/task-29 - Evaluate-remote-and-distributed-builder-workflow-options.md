---
id: TASK-29
title: Evaluate remote and distributed builder workflow options
status: To Do
assignee: []
created_date: '2026-05-28 00:41'
updated_date: '2026-05-28 00:49'
labels:
  - exploratory
  - builds
  - performance
dependencies: []
references:
  - AGENTS.md
  - scripts/remote_config.sh
  - docs/COMPILE_PERFORMANCE_PLAN.md
priority: medium
ordinal: 23000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Assess tools and workflows that can reduce local MacBook build load across several active projects, including the existing jcode remote build scripts and broader options such as Nix remote builders, sccache, cargo cache sharing, and dedicated build hosts. This is partially existing workflow rather than a brand-new roadmap item: jcode already documents and ships remote build support.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Existing jcode remote build support is summarized with setup requirements, current limitations, and when to use it.
- [ ] #2 Candidate cross-project tools are compared for Rust/Cargo, Nix, and mixed-project workloads, including setup cost and operational risk.
- [ ] #3 Recommendations identify quick wins versus heavier infrastructure that may be more work than it is worth for a small local network.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Research notes: remote/distributed builders are clearly viable and mature. Nix has official distributed build support over SSH with configured build machines and builders-use-substitutes; sccache supports shared caching and distributed Rust compilation, including macOS clients with Linux build servers, but toolchain matching and setup complexity matter. Cargo officially recommends sccache as the shared-cache path. For jcode, existing scripts/remote_config.sh and remote build guidance already cover part of this, so the best next step is comparing quick wins: Nix remote builders for Nix builds, sccache shared cache or sccache-dist for Cargo, and the existing jcode remote build path for selfdev builds.

Implementation sketch: treat builders separately from agent workers. Add a small config/discovery layer that can report available build backends: existing jcode remote build, Nix buildMachines, sccache shared cache, and optional sccache-dist. A one-shot build command should choose the backend explicitly first, then later support auto-selection based on project type, current load, and estimated job cost. Keep build artifacts/cache management in native tools rather than reinventing a jcode scheduler.
<!-- SECTION:NOTES:END -->
