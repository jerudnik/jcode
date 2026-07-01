# Jcode integration observations

Owner: Jcode

Use this log for implementation-facing observations about how the control plane should integrate with jcode internals.

## How to add an observation

```md
## YYYY-MM-DD - Short title

- Context:
- Existing jcode surface involved:
- Integration implication:
- Candidate data model/API:
- Risk or migration concern:
- Validation idea:
```

## Initial observations

## 2026-07-01 - Existing primitives already resemble a control plane

- Context: jcode already has background tasks, swarm coordination, scheduling, debug sockets, side panels, session search, and mobile/server surface work.
- Existing jcode surface involved: `bg`, `swarm`, `schedule`, server client APIs, protocol events, mobile server manager, gateway access modes.
- Integration implication: the first control-plane layer should normalize existing primitives rather than introduce a parallel orchestration system.
- Candidate data model/API: `Run`, `Actor`, `Surface`, `Event`, `Command`, and `Approval` records, backed by append-only events plus current-state snapshots.
- Risk or migration concern: duplicating task state between ad hoc tools and the control plane could create stale views or unsafe controls.
- Validation idea: build a read-only inventory endpoint first, then compare it against current `bg`, `swarm status`, and session lists.

## 2026-07-01 - Local-first durability should come before remote control

- Context: reloads, shell exits, background task completion, and multi-surface handoff are common failure points.
- Existing jcode surface involved: background task status files, ambient scheduling, selfdev reload, gateway/mobile proposals.
- Integration implication: control-plane state should survive process reload and machine sleep before it is exposed remotely.
- Candidate data model/API: append events to a local store with monotonic IDs, periodic snapshots, and explicit recovery events after restart.
- Risk or migration concern: remote/mobile control without robust local recovery would make failures harder to reason about.
- Validation idea: create a state-space test that kills and restarts the control-plane process while tasks transition through queued/running/waiting/done.

## 2026-07-01 - 4nix should own host policy, jcode should own behavior

- Context: 4nix wires services, packages, caches, host secrets, and desktop/mobile launch policy, while jcode owns agent behavior and protocol semantics.
- Existing jcode surface involved: Nix flake/Home Manager module, gateway secure access modes, spawn hook/router integration.
- Integration implication: jcode should expose clear control-plane services and config knobs; 4nix should decide where and how those services run on a host.
- Candidate data model/API: jcode exports service command, socket path/env contract, and auth mode options; 4nix maps them into launchd/systemd/Home Manager.
- Risk or migration concern: putting host-specific policy inside jcode would make the fork less reusable and harder to test.
- Validation idea: document a minimal service contract and ensure it can be implemented both by the flake module and by a manual local command.

