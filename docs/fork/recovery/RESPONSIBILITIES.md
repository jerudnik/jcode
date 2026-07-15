# Responsibility index

This is a provisional map, not a conclusion. Luna creates the first researched map, Sonnet improves it, and the coordinator approves the boundaries and review depth before seam teams begin. Split or merge rows when evidence shows that behavior, ownership, or validation cannot be decided coherently as one unit.

| ID | Responsibility | Key surfaces | Review | State | Disposition | Ledger |
|---|---|---|---|---|---|---|
| R00 | Integration lineage and sync governance | refs, ancestry, patch equivalence, sync policy | `untriaged` | `seed` | `undecided` | pending |
| R01 | Live runtime identity and reload authority | build hash, daemon registry, selfdev, handoff | `untriaged` | `seed` | `undecided` | pending |
| R02 | Configuration, auth, providers, and routing | config provenance, credentials, model selection, sidecars | `untriaged` | `seed` | `undecided` | pending |
| R03 | Client/server protocol compatibility | wire types, handshake, reconnect, version policy | `untriaged` | `seed` | `undecided` | pending |
| R04 | Session lifecycle and supervision | create, resume, cancel, shutdown, recovery, backoff | `untriaged` | `seed` | `undecided` | pending |
| R05 | Swarm, comm, DAG, and scheduling | task graph, control log, run-plan, workers, liveness | `untriaged` | `seed` | `undecided` | pending |
| R06 | Persistence, evidence, memory, and replay | session store, journals, snapshots, backups, provenance | `untriaged` | `seed` | `undecided` | pending |
| R07 | Tools, MCP, discovery, telemetry, and network policy | tool registry, MCP lifecycle, reporting, consent | `untriaged` | `seed` | `undecided` | pending |
| R08 | TUI and CLI interaction surfaces | input, rendering, cards, commands, operator feedback | `untriaged` | `seed` | `undecided` | pending |
| R09 | Tests, CI, and quality gates | parsers, ratchets, fixtures, inherited-red policy | `untriaged` | `seed` | `undecided` | pending |
| R10 | Packaging, release, update, and distribution | Nix, wrappers, channels, updater, release metadata | `untriaged` | `seed` | `undecided` | pending |
| R11 | Documentation, incidents, and backlog governance | active docs, maintenance state, stale instructions | `untriaged` | `seed` | `undecided` | pending |

## Index editing rules

- Keep each responsibility name and surface summary short enough to scan without wrapping excessively.
- Set `Review` to `full`, `light`, or `defer` after the mechanical divergence and risk pre-screen.
- Use full Opus/Grok/Terra review for at most six seams at once. Rank by divergence, operational risk, protected invariants, and pilot dependency.
- Use a lightweight ledger for low-risk or mechanically equivalent seams. Fable may escalate any light seam.
- Link `Ledger` to `seams/<ID>-<slug>/ledger.md` once the directory exists.
- Put evidence, recommendations, authorship, and debate in the seam ledger, not this table.
- The coordinator is the only writer during parallel research. Seam teams propose index changes in their ledger.
