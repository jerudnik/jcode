---
id: TASK-23
title: Design pure flake outputs for portable agent data
status: To Do
assignee: []
created_date: '2026-05-27 17:54'
labels:
  - planning
  - flake
milestone: m-0
dependencies:
  - TASK-1
  - TASK-2
references:
  - flake.nix
  - docs/
documentation:
  - backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md
priority: medium
ordinal: 3000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan a minimal flake surface that exposes this repository as pure data and checks for consumers, while leaving installation and runtime policy in nix-config.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Proposed outputs are limited to data packages, catalogs, apps, or checks with no host deployment policy.
- [ ] #2 Design explains how consumers can access skills, prompts, permissions, specs, and validation results.
- [ ] #3 Non-goals explicitly exclude secrets, services, home-manager activation policy, and nix-config lock updates.
<!-- AC:END -->
