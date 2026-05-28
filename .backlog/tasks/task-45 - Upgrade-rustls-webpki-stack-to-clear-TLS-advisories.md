---
id: TASK-45
title: Upgrade rustls/webpki stack to clear TLS advisories
status: To Do
assignee: []
created_date: '2026-05-28 05:03'
labels:
  - security
  - dependencies
  - rustls
  - webpki
  - tls
  - supply-chain
dependencies: []
references:
  - 'docs/SECURITY_DEPENDENCIES.md:12-20@0aea41ac'
  - 'commit:0aea41ac'
priority: high
ordinal: 39000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Move rustls/webpki to current secure versions across all crates. Verify HTTP client and websocket paths.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 rustls and webpki updated repo-wide
- [ ] #2 cargo audit reports no related advisories
- [ ] #3 Reqwest and tokio-tungstenite paths verified
<!-- AC:END -->
