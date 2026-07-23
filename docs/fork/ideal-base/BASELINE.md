# Ideal-base starting boundary

Recorded: 2026-07-18

This file carries forward only the facts required to execute the ideal-base
program safely. Historical reconstruction remains under
[`../normalization/`](../normalization/) and [`../recovery/`](../recovery/).

## Source and runtime seed

These facts were true when the railway was created and must be revalidated at the
start of every coordinator session:

- Canonical checkout: `/Users/jrudnik/labs/jcode`.
- Canonical branch: `main`.
- Railway seed source commit: `923c6353e04266f71dc6cc06fc8516e502a9c07f`.
- The seed source was clean and two commits ahead of `origin/main`.
- Product/runtime commit: `8962bccb32eede3b6746c42bfe6d265df29e4471`.
- Runtime label: `8962bccb3-release`.
- Runtime SHA-256: `6cf81221e8c0cee86ae714d2f1fc9fb55fe8715f45ee8082dc2ecf034a2515fc`.
- `current`, `stable`, and `shared-server` selected that exact immutable release.
- The self-development manifest had no canary or pending activation.
- One registered Git worktree remained.
- Recovery refs, four stashes, rollback bundles, sealed evidence, and private
  archives remained preserved.
- No provider request, live publication, push, or platform-gated validation was
  performed while creating this railway.

The authority commit for this infrastructure cannot self-identify inside its own
contents. Derive it with:

```bash
git log -1 --format='%H' -- docs/fork/ideal-base/WORK_GRAPH.json
```

If current facts differ, record the observation in [`DECISIONS.md`](DECISIONS.md)
or a node artifact, update `STATE.json` only when justified, and stop before any
unsafe mutation.

## Protected historical assets

These boundaries are non-negotiable unless the user explicitly authorizes a
separate migration:

- Do not delete or rewrite recovery refs, stashes, bundles, sealed evidence,
  private archives, or historical ledgers.
- Do not move evidence packages merely to simplify the documentation layout.
- Do not alter `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`.
- Expected protected prompt SHA-256:
  `ca3f19980b1e4fab0a734397d7c6f41ccd5c203a4fa209cfe9eef2f16beed5b6`.
- Do not rewrite historical facts to match current state. Add a superseding active
  record instead.
- Do not push without explicit authorization.

## Accepted current label

The current checkout is **core-runtime validated and suitable for regular TUI/CLI
feature development**. It is not yet an ideal-base signoff and does not imply
complete provider, mobile, WebSocket, packaging, updater, unattended swarm, or
platform coverage.

## Source-verified remaining classes

The mandatory deterministic work is grouped into five classes:

1. Runtime ownership and persistence: server activity/shutdown authority,
   background-task status durability, MCP child ownership, and dead/hung MCP
   recovery.
2. Recovery and resource bounds: pending activation reconciliation, disconnect
   terminal reconciliation, and global subresource caps.
3. Deterministic validation and packaging: real-process lifecycle tests, blocking
   Linux/macOS/TUI rails, real Nix package builds, installed mobile assets, and
   updater/reload acquisition fixtures.
4. Hardening and cleanup: security and quality ratchets, provenance, socket and
   durable-state hygiene, PID sweeps, telemetry liveness, and dead-source cleanup.
5. Gated validation: `aarch64-linux`, providers/catalog, mobile/iOS, Windows,
   FreeBSD, and live release/update paths.

Exact node dependencies and gates are in [`WORK_GRAPH.json`](WORK_GRAPH.json).
