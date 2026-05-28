---
id: TASK-30
title: Explore context-aware natural-language shell and text completions
status: To Do
assignee: []
created_date: '2026-05-28 00:41'
updated_date: '2026-05-28 12:20'
labels:
  - exploratory
  - ux
  - completion
  - feature
  - UX
  - security
  - performance
  - shell
  - nlp
dependencies: []
references:
  - crates
  - README.md
  - >-
    .backlog/tasks/task-30 -
    Explore-context-aware-natural-language-shell-and-text-completions.md:27@0aea41ac
  - 'commit:0aea41ac'
  - 'Serena memory: compaction/dcp_research_task27'
priority: low
ordinal: 24000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Investigate whether a small jcode-connected extension or background service could turn natural-language requests into shell completions or system-wide text completions, similar in spirit to cotypist/cotabby, without compromising latency, privacy, or safety. This is exploratory and may be better as an external companion tool than core jcode.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Possible integration surfaces are identified, including shell completion hooks, terminal integration, editor plugins, macOS text services, accessibility APIs, and a jcode server client.
- [ ] #2 A minimal prototype scope is proposed with latency, consent, privacy, and command-safety constraints.
- [ ] #3 The analysis recommends whether this belongs in core jcode, a separate extension, or should be deferred as too broad.
- [ ] #4 Prototype scope considers using repo maps, active-task handles, trust tiers, and just-in-time context references rather than injecting broad shell/session history.
- [ ] #5 Latency/privacy analysis includes token-budgeted context selection and quarantining unverified or stale shell suggestions.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Expanded from TASK-27 research: context-aware completions should reuse the same just-in-time context references, trust tiers, and token-budgeted selection principles rather than broad eager context injection.
<!-- SECTION:NOTES:END -->
