---
id: TASK-46
title: Replace or isolate imap / imap-proto dependency
status: To Do
assignee: []
created_date: '2026-05-28 05:03'
labels:
  - security
  - dependencies
  - imap
  - email
  - supply-chain
  - isolation
dependencies: []
references:
  - 'docs/SECURITY_DEPENDENCIES.md:12-20@0aea41ac'
  - crates/jcode-notify-email/Cargo.toml@0aea41ac
  - 'commit:0aea41ac'
priority: high
ordinal: 40000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Investigate upgrading or replacing imap/imap-proto. If no maintained path exists, isolate or remove the IMAP dependency from the default build.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A decision is recorded: upgrade, replace, isolate, or remove
- [ ] #2 Default build does not depend on an unmaintained imap stack
- [ ] #3 cargo audit clean for imap-related crates in default features
<!-- AC:END -->
