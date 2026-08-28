---
title: "Swarm evidence logs still label direct provider routes as OpenRouter"
status: open
priority: high
owner: unassigned
opened: 2026-08-27
related:
  - docs/issues/swarm-observability-status-and-wake-gaps.md
  - docs/architecture/provider-confusion.md
---

# Swarm evidence logs still label direct provider routes as OpenRouter

The spawn routing fixes work, but the evidence log still records the wrong
provider for a direct OpenAI-compatible route. This leaves the four identity
surfaces inconsistent, so the issue remains open.

## Current verification

Verified on 2026-08-28 with Jcode `fc76f06f2`. The running binary and repository
HEAD matched, and the three provider-identity fixes from pull request 215 were
ancestors of that revision.

A worker was spawned with the exact catalog name `glm-5.2`. The spawn result was:

```text
Resolved identity: model=glm-5.2 provider_key=zai route=openai-compatible:zai
```

The worker completed a real tool-backed task. It read this issue with two shell
calls, reported the final heading and line count, and returned the correct short
Git revision. The turn finished successfully in 51.315 seconds.

The degradation seen before the fixes did not recur. Four provider responses
completed in 10.883 to 14.767 seconds, each with nonzero input and output token
counts. The worker completed instead of repeatedly dropping orphaned tool
outputs and timing out.

## Remaining identity mismatch

For worker session `session_hamster_1787909831452_aab3c62d7978efae`:

| Surface | Provider | Model | Result |
| --- | --- | --- | --- |
| Spawn tool result | `zai` | `glm-5.2` | correct |
| Persisted session meta and all journal snapshots | `zai` | `glm-5.2` | correct |
| Evidence `provider_request` and `provider_response` events | `OpenRouter` | `glm-5.2` | wrong provider |
| TUI swarm runtime shown by the live swarm status | `Z.AI` | `glm-5.2` | correct |

The TUI adapter in
`crates/jcode-tui/src/tui/info_widget_swarm_gallery.rs` renders the chip from
`SwarmMemberStatus.runtime.provider` and `.model`, the same live runtime fields
reported as `Z.AI / glm-5.2` during this check.

The evidence writer is the only disagreeing surface. It appears to report the
generic OpenRouter-compatible transport implementation instead of the selected
provider profile.

## Route prompt check

`swarm list_models` now lists both `glm-5.2` and `deepseek-v4-pro`, exactly as
`.jcode/swarm-prompt.md` recommends them. The earlier resolution failures were
from the pre-fix resolver. The prompt is currently accurate and was not changed.

## Acceptance

This issue can be deleted when a named-route worker completes a real task and
all four identity surfaces report the selected provider and model. The check
must also confirm successful responses with nonzero token accounting and no
repeated orphaned-tool-output warnings.
