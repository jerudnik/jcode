---
id: TASK-30
title: Explore context-aware natural-language shell and text completions
status: To Do
assignee: []
created_date: '2026-05-28 00:41'
labels:
  - exploratory
  - ux
  - completion
dependencies: []
references:
  - crates
  - README.md
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
<!-- AC:END -->
