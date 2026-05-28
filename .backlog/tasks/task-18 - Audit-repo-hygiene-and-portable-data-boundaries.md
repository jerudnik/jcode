---
id: TASK-18
title: Audit repo hygiene and portable-data boundaries
status: Done
assignee:
  - '@jcode-agent'
created_date: '2026-05-27 17:54'
updated_date: '2026-05-28 02:49'
labels:
  - planning
  - hygiene
milestone: m-2
dependencies: []
references:
  - AGENTS.md
  - .gitignore
  - serena/
documentation:
  - >-
    .backlog/docs/planning/doc-2 -
    Repo-hygiene-and-portable-data-boundary-audit.md
priority: high
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Inventory tracked, generated, runtime, and local-only files so the repo remains portable agent data before any flake work begins. Document cleanup needs rather than touching runtime state.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Repository content is categorized into portable source data, generated artifacts, local runtime state, and deployment-policy concerns.
- [x] #2 Cleanup recommendations identify ignore-rule or documentation updates without deleting runtime files.
- [x] #3 Boundary notes explicitly state that secrets and runtime services remain outside this repo.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Self-improvement loop:
1. Review roadmap doc, AGENTS.md, gitignore, and tracked/untracked repository content.
2. Categorize portable source data, generated artifacts, local runtime state, and deployment-policy concerns.
3. Identify cleanup recommendations limited to ignore-rule or documentation updates; do not delete runtime files.
4. Add a durable backlog document capturing the boundaries and recommendations.
5. Validate the documentation is discoverable and mark acceptance criteria complete.
6. Commit and push documentation/task metadata.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Created doc-2 with the repository hygiene and portable-data boundary audit. Inventory found ignored local runtime/build state only: target/, .direnv/, and scripts/__pycache__/. No runtime files were deleted.

Tracked content was categorized into portable source data, generated/intentional assets, local runtime state, and deployment-policy concerns. Boundary notes explicitly keep secrets, runtime services, logs, build installs, daemon state, and host service definitions outside this repo.

Validation: backlog doc list finds doc-2; MCP document_view returned the full persisted document.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Completed the repo hygiene and portable-data boundary audit for the flake transition foundation.

Changes:
- Added Backlog document doc-2: .backlog/docs/planning/doc-2 - Repo-hygiene-and-portable-data-boundary-audit.md.
- Categorized tracked content into portable source data, generated/intentional artifacts, local runtime state, and deployment-policy concerns.
- Recorded cleanup recommendations focused on ignore-rule/documentation follow-ups only; no runtime files were deleted.
- Explicitly documented that secrets, runtime services, logs, build installs, daemon state, and host service definitions remain outside this repo and belong to local runtime locations or consumer deployment repos such as nix-config.

Validation:
- backlog doc list finds doc-2.
- MCP document_view returned the persisted doc content.
<!-- SECTION:FINAL_SUMMARY:END -->
