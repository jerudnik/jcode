---
id: TASK-66
title: 'Safety system Phase 2: notification channels'
status: To Do
assignee: []
created_date: '2026-05-28 05:06'
labels:
  - security
  - safety
  - notifications
  - desktop
  - email
  - webhook
  - sms
dependencies:
  - TASK-65
references:
  - 'docs/SAFETY_SYSTEM.md:519-523@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 60000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Desktop notifications, SMTP email, webhook, batching + quiet hours, SMS provider (Twilio or similar).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each channel can deliver a test notification
- [ ] #2 Batching and quiet hours respected
- [ ] #3 Channels are independently configurable
<!-- AC:END -->
