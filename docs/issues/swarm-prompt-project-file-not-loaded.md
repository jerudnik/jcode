---
title: "Project swarm-prompt.md is resolved against the server cwd, not the session working dir"
status: open
priority: medium
owner: unassigned
opened: 2026-09-21
related:
  - crates/jcode-base/src/prompt.rs
  - crates/jcode-app-core/src/tool/communicate.rs
  - .jcode/swarm-prompt.md
---

# Project swarm-prompt.md is resolved against the server cwd, not the session working dir

`CommunicateTool::new()` calls `load_swarm_prompt(None)`, and with `None` the
loader resolves `./.jcode/swarm-prompt.md` against the process working
directory. The long-running server (`jcode serve`, launched by the sentinel)
runs with cwd `/`, so the project lookup always misses and the loader falls
through to `~/.jcode/swarm-prompt.md`.

Observed on 2026-09-21 with build `ffa646a60`: the session working directory
was the jcode repository, the project file existed, and the swarm tool
description still carried the global file's content (a stale campaign section
from 2026-09-01). `docs/agent-workflows.md` names `.jcode/swarm-prompt.md` as
the only repository authority for routing, and the `/swarm-prompt` command
edits that file, so users reasonably expect it to take effect.

## Desired behavior

The project prompt resolves against the session's working directory (the
registry already knows it through `ToolContext`), and the description
refreshes when that file changes or at least on each registry construction
for the session's own directory.

## Workaround

Keep `~/.jcode/swarm-prompt.md` in sync with the repository file by hand.
