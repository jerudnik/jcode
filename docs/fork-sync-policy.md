# Fork harvest policy

Status: **superseded in part.** Since the 2026-07-27 hard fork there is no
periodic sync, so the per-sync cadence below no longer runs. See
[`fork/SYNC_MODEL.md`](./fork/SYNC_MODEL.md) for the current model and
[`BRANCHING.md`](./BRANCHING.md) for the rails.

What survives is the *judgement*: how to decide whether an individual upstream
commit is worth cherry-picking, and what must never be traded away to take one.
Read this before harvesting anything from `upstream`.

Upstream is a parts bin, not gospel. That posture predates the hard fork:
upstream master's own CI was red (e2e `Server disconnected` failures in the
v0.46/v0.47-prep window), and several upstream changes removed or fought seams
this fork relies on.

## Process per harvest

### (a) Inspect the upstream delta

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

## Test and gate policy during a harvest

- **Applying cleanly is not evidence of correctness.** A patch that does not
  textually conflict can still be wrong here: at the hard fork, one cleanly
  applying commit added two modules whose call sites lived in a fork-modified
  file, so taking it alone would have left orphaned dead code.
- **Run `scripts/preflight.sh` after every harvest.** Upstream code routinely
  fails this fork's gates. Of the eight commits harvested at fork declaration,
  three needed follow-up fixes (an eight-argument function, a swallowed error,
  a file pushed over the code-size ceiling).
- **Fix the harvested code; never raise the budget to admit it.** If an
  imported file breaks a ratchet, split or repair it.
- Never weaken a fork test to make upstream code fit. Either adapt the code or
  skip the upstream change.
- Use `git cherry-pick -x` so the upstream origin is recorded in the message.

## Fork seams (do not let upstream erode these)

- `subagent_type` + `initial_prompt_delivered` swarm member plumbing.
- Channel pub/sub REMOVED (3dd11cf3c) and shared-context retired for
  `PlanProposalCache` (a3619bf9a). Upstream still carries both; strip them from
  merged code.
- APM/`.apm` skill + MCP manifest seams ("Fork seam" comments).
- `control_log_covered_offset` swarm persistence field.
- Assistant profiles config.
- Nix-managed packaging (`JCODE_NIX_MANAGED`, no auto-update).

## Harvest ledger

### 2026-07-27 (hard fork declaration, final harvest)

Of 678 upstream commits since the fork point, 52 touched only files this fork
had never modified and 20 of those cherry-picked cleanly. Eight were taken:

Adopted: openrouter SSE multi-data-line + CRLF boundary fix (#565), antigravity
`thought_signature` self-heal (#482, #518), webfetch output capping and chrome
stripping (~4x context reduction), bounded/reused external session history,
terminal-image suppression on redirected stdout, onboarding credential sandbox
isolation, `clean_target.sh --sweep`, and an OpenAI system-prompt pass-through
test.

Skipped: the `jcode-desktop2` skeleton and its follow-ups (a parallel UI stack
this fork has no plan to carry), Windows/release plumbing and the Discord
announcement workflow (this fork ships to macOS and Nix; the Windows workflows
are already dispatch-only), upstream's telemetry worker SQL
(this fork does not operate that endpoint; see `TELEMETRY.md`), the sponsor
attribution benchmark, and `refactor(update): split metadata and rate-limit
state out of update.rs` (adds two modules but leaves their wiring in a
fork-modified `update.rs`, so it applies cleanly and produces dead code).

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
