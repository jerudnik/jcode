# Fork sync policy

Status: active since the v0.46 sync (2026-07-14). Supersedes "track upstream closely."

The fork and upstream (1jehuang/jcode) have diverged deliberately. Upstream is a
parts bin, not gospel. Evidence for the posture shift: upstream master's own CI
is red (e2e `Server disconnected` failures shipped in the v0.46/v0.47-prep
window), and several upstream changes have removed or fought seams this fork
relies on.

## Process per sync

### (a) Inspect upstream delta since last sync

Classify every change (commit or feature-cluster):

- fixes a bug?
- introduces a new feature?
- removes a feature?
- hardens or otherwise improves a core process?
- addresses a security issue?
- something else (refactor, style, release chore)?

### (b) Judge each change against the fork

For each change, ask about its desirability **on the fork**:

- Does it fix a problem we actually have?
- If we already solved that problem, is upstream's solution more elegant than
  ours? Adopt the better one; drop the other.
- Is the benefit worth the integration cost?

If there is benefit, adopt it by whichever route fits:
1. verbatim merge,
2. massaging upstream's code to fit our architecture, or
3. massaging our fork to fit upstream's architecture (when theirs is genuinely
   better).

If a change doesn't make sense, or deliberately tears out something we rely on
without offering (or at least hinting at) a more elegant replacement, treat it
with skepticism. Skipping it is a valid outcome. Record skipped changes below so
future syncs don't re-litigate them.

## Test policy during sync

- Done-condition for a sync is **fork-main parity plus adopted features
  working**, not "all tests green." Upstream ships red tests; inheriting them
  is not a regression on our side.
- Known inherited-red tests must be listed in the sync commit message with
  evidence (upstream CI run link).
- Never weaken a fork test to make upstream code fit. Either adapt the code or
  skip the upstream change.

## Fork seams (do not let upstream erode these)

- `subagent_type` + `initial_prompt_delivered` swarm member plumbing.
- Channel pub/sub REMOVED (3dd11cf3c) and shared-context retired for
  `PlanProposalCache` (a3619bf9a). Upstream still carries both; strip them from
  merged code.
- APM/`.apm` skill + MCP manifest seams ("Fork seam" comments).
- `control_log_covered_offset` swarm persistence field.
- Assistant profiles config.
- Nix-managed packaging (`JCODE_NIX_MANAGED`, no auto-update).

## Sync ledger

### 2026-07-14 (upstream v0.45–v0.46, 221 commits)

Adopted: SwarmMemberRuntime, terminal-member GC, required spawn labels,
RuntimeTaskScope cancellation, NS1 protocol/build handshake, required absolute
Subscribe working_dir (good hygiene: fixes daemon-cwd leak), LaTeX image
rendering, multiline-math hardening, fork_for_new_session, device WS auth
tests, OpenRouter catalog gating (kept alongside fork's
resolve_current_model_spec).

Skipped/overridden: channel pub/sub reintroduction (fork removed it),
shared-context (fork replaced with PlanProposalCache), upstream's
oauth_format_tools_keeps_full_custom_toolset test (incompatible with fork's
allowlist semantics).

Inherited-red: upstream e2e suite fails on upstream's own CI (working-dir
handshake landed without updating test clients). Fork fixes the test support
instead of inheriting the red.
