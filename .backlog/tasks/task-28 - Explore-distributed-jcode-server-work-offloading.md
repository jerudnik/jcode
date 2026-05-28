---
id: TASK-28
title: Explore distributed jcode server work offloading
status: To Do
assignee: []
created_date: '2026-05-28 00:41'
updated_date: '2026-05-28 00:49'
labels:
  - exploratory
  - architecture
  - distributed
dependencies: []
references:
  - README.md
  - crates
  - jcode-build-support/src/lib.rs
priority: medium
ordinal: 22000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Investigate whether jcode server/session architecture could safely distribute selected work across multiple trusted machines on a local network, such as background jobs, low-urgency subagents, or tool execution, while returning coherent results to the user-facing session. This is speculative: plain internet/RPC can move work between machines, but correctness depends on state synchronization, trust boundaries, cancellation, file access, and result merging.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Current jcode server/session/background-task architecture is mapped to identify what is stateful, latency-sensitive, or safe to offload.
- [ ] #2 Candidate offload categories are ranked by feasibility, including tool calls, background processes, subagents, builds, and model/provider requests.
- [ ] #3 Risks and non-goals are documented for network security, filesystem consistency, secrets, cancellation, retries, and merging remote results into the local UI.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Research notes: distributed agent runtimes are viable in principle. AutoGen documents an experimental host/worker gRPC runtime where a host maintains worker connections, message delivery, and direct-message/RPC sessions; workers advertise supported agents and process application code. This supports the idea that jcode could eventually route selected jobs to trusted LAN workers, but the hard parts are session consistency, state ownership, filesystem access, secrets, cancellation, retries, and result merging. A safer first prototype is a one-shot remote worker command that accepts stdin/request JSON and returns stdout/JSON, rather than transparent distributed interactive sessions.

Implementation sketch: start with a local-first Unix filter command rather than full distributed sessions. Current CLI already has Command::Run routed through run_single_message_command, which creates an Agent directly, supports --resume, --json, and --ndjson, and uses run_once_capture/run_once_streaming_mpsc. Step 1 could add stdin support and a clearer alias such as jcode ask/pipe that combines prompt args plus piped stdin. Step 2 could add a local server-backed request path that sends a one-shot request to an existing jcode server over the current JSON-line socket protocol. Step 3 could add trusted remote worker profiles over SSH by executing jcode run/ask remotely with JSON input and JSON/NDJSON output. Only after that should we consider a native worker daemon with capability registration, leases, timeouts, and structured job results.
<!-- SECTION:NOTES:END -->
