# Current fork normalization status

Recorded: 2026-07-18

This is the current operating checkpoint for the recovery-to-normalization
program. It supersedes pre-promotion and pre-cleanup facts in `BASELINE.md`,
`COORDINATOR_BRIEF.md`, and earlier versions of this file without rewriting
those historical records.

## Current source and runtime

- Canonical checkout: `/Users/jrudnik/labs/jcode`.
- Canonical branch: `main`.
- Source checkpoint before this amendment:
  `152ececcc57c153731685ff398352a4494bd679b`.
- Product/runtime commit:
  `8962bccb32eede3b6746c42bfe6d265df29e4471`.
- Runtime label: `8962bccb3-release`.
- Runtime SHA-256:
  `6cf81221e8c0cee86ae714d2f1fc9fb55fe8715f45ee8082dc2ecf034a2515fc`.
- `current`, `stable`, and `shared-server` all select that exact immutable
  release. No runtime reload or promotion was performed during this checkpoint.
- Self-development manifest: stable projection normalized to the release, with
  no canary and no pending activation.
- Preserved prompt disposition: `docs/fork/recovery/ORCHESTRATOR_PROMPT.md` is
  restored to its tracked baseline and retained as historical recovery evidence
  and an architectural example. Fifty-one documentation files reference it.
- Removed local residue: untracked `opencode.json`, auxiliary worktrees, and
  rebuildable Cargo output after validation.

The signed recovery source and sign-off commits remain preserved on the recovery
line, under archive refs, and in verified rollback archives. They are not
ancestors of curated `main`; commit `c786be6c3` imported the forensic record
into curated history.

## Milestone and gate disposition

| Surface | Status | Evidence and boundary |
|---|---|---|
| N0 safety inventory | Complete | Verified all-ref and stash bundles plus committed N0 evidence. |
| N1 curated integration | Complete | Curated tree and recovery-equivalence evidence preserved. |
| N2 W7 and promotion | Complete with recorded post-signoff fixes | Original signoff at `62b3946b6`; operational fixes at `1c368592f` and `8962bccb3` are recorded in `N2_SIGNOFF.md`. |
| Runtime promotion | Complete | Exact release, channel identity, daemon identity, subscribed ping, and reload behavior verified. |
| N3 documentation and task normalization | Complete for the current baseline | Historical ledgers remain evidence; current state and remaining seams are in `KNOWN_GOOD_BASELINE.md`. |
| N4 isolated runtime matrix | Complete for core runtime | Final sealed campaign passed 321/321 checks with no provider requests, leaked sandbox processes or sockets, or live-state mutation. Provider-specific turns remain authorization-gated. |
| N5 host normalization | Complete for source/build residue | One canonical worktree remains; rebuildable Cargo output is removed after validation. Refs, stashes, bundles, sealed evidence, and private archives remain preserved. |
| N6/D9 final sign-off | Core-runtime complete; unqualified sign-off open | Regular TUI/CLI feature work may proceed. Real-provider validation and the ranked MCP/headless lifecycle seams remain explicit boundaries. |

The honest label is **core-runtime validated and ready for regular TUI/CLI
feature development**. It is not an unqualified claim that every provider,
mobile, WebSocket, packaging, or unattended-swarm path is closed.

## Known-good evidence

The authoritative current inventory is
[`KNOWN_GOOD_BASELINE.md`](KNOWN_GOOD_BASELINE.md). The sealed stress package is
at `~/labs/.recovery/jcode/2026-07-17-runtime-stress/`; its `SHA256SUMS` file has
SHA-256
`24366cbb5d58c22b5b3ef24ad19b434aa61cb1ddfda763770f823f6fd5c61ae4`.
The bounded operational cleanup and final-source live validation are preserved
at `~/labs/.recovery/jcode/2026-07-18-known-good-baseline/`.

## Private archive and local preservation

The private recovery repository remains:

<https://github.com/jerudnik/jcode-recovery-archive>

It preserves branch history but does not replace local rollback bundles,
stashes, sealed evidence, or recovery archives. Those assets remain local and
unchanged. No stash, bundle, recovery ref, or private archive was deleted or
uploaded during this checkpoint.

## Next engineering checkpoint

The next high-value work is not more recovery cleanup. It is the bounded
lifecycle remediation already outlined in
`docs/proposals/swarm-lifecycle-remediation.md`:

1. explicitly reap owned MCP children on every daemon exit path and give
   `mcp-serve` an owner-liveness fallback;
2. make daemon idle exit respect active headless, swarm, debug-job, background,
   and MCP work;
3. evict and reconnect dead shared MCP children.

Each item has a deterministic no-provider acceptance gate in
`KNOWN_GOOD_BASELINE.md`. No push, runtime promotion, real-provider request, or
archive deletion is authorized by this status update.
