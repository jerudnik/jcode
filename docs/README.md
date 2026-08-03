# Documentation map

446 tracked Markdown files sit in this repository, and they do not all carry the
same weight. Some are contracts you must follow, some are proposals nobody has
accepted, and some are frozen forensic records that were true once and are
deliberately never updated. Reading a proposal as a contract is the failure this
map exists to prevent.

Authority is stated per tier below. When two documents conflict, the higher tier
wins, and the lower one is the defect.

## Tier 1: binding contracts

Generated `AGENTS.md` files and the primitives that produce them. These bind
agent behavior and are enforced by `scripts/check_agent_instructions.py`.

| Path | Owns |
| --- | --- |
| `AGENTS.md` (root) | the repository work contract |
| [`AGENTS.md`](./AGENTS.md) | documentation maintenance and integration |
| [`agent-workflows.md`](./agent-workflows.md) | the operational commands and gates |
| [`BRANCHING.md`](./BRANCHING.md) | branch rails and fork invariants |
| [`NIX.md`](./NIX.md) | the packaging and distribution rail |
| `.apm/instructions/*.instructions.md` | the source of every generated `AGENTS.md` |

Never edit a generated `AGENTS.md`. Edit its primitive and run `apm compile`.

## Tier 2: current subsystem documentation

Describes what the code does now. If one of these disagrees with the code, that
is a bug in the document or in the code, and it must be resolved rather than
narrated. Softening the prose to match a shortfall is not an allowed repair;
file a product repair instead.

Substantial current surfaces, not exhaustive:

| Area | Documents |
| --- | --- |
| Providers and auth | `AUTH_CREDENTIAL_SOURCES.md`, `PROVIDER_DOCTOR.md`, `AWS_BEDROCK_PROVIDER.md`, `../OAUTH.md` |
| Sessions and server | `SERVER_ARCHITECTURE.md`, `SERVER_LIFECYCLE_INVARIANTS.md`, `RESUME_BEHAVIOR.md` |
| Memory | `MEMORY_ARCHITECTURE.md`, `MEMORY_BUDGET.md` |
| Swarm | `SWARM_ARCHITECTURE.md`, `SWARM_TASK_GRAPH.md` |
| Tools and hooks | `AGENT_TOOL_INTEGRATION.md`, `HOOKS.md`, `SPAWN_HOOK.md` |
| Telemetry and security | `../TELEMETRY.md`, `SECURITY_DEPENDENCIES.md`, `INTERACTION_SURFACE_SECURE_ACCESS.md` |
| Platform | `WINDOWS.md`, `WRAPPERS.md`, `NIX.md` |

`docs/architecture/` (22 files) holds deeper design records for the same
surfaces.

## Tier 3: proposals and drafts

Not authority. A document here describes what someone wanted, not what shipped.
Several self-declare `Status: Proposed` or `Status: Draft`; the absence of such a
line does not promote a proposal into a contract.

`docs/proposals/` (17 files), plus top-level documents that declare a
non-current status, including `MODULAR_ARCHITECTURE_RFC.md`,
`MERMAID_RENDERING_REDESIGN.md`, `MULTI_SESSION_CLIENT_ARCHITECTURE.md`,
`DESKTOP_APP_ARCHITECTURE.md`, `DESKTOP_CODEBASE_ARCHITECTURE.md`, and
`DESKTOP_SUPERAPP_WORKSPACE.md`.

Only 17 of 57 top-level documents carry a `Status:` line at all, so treat a
missing status as unclassified rather than as current.

## Tier 4: frozen records

Historical by design. These are **not** kept in sync with the code, and
correcting them would destroy their value as evidence.

| Path | Why frozen |
| --- | --- |
| `docs/archive/` (11 files) | superseded material |
| `docs/fork/recovery/` (131 files) | forensic integrity, self-declared |
| `docs/fork/normalization/` (26 files) | append-only snapshot, self-declared |
| `docs/fork/ideal-base/` (152 files) | program ledger; `DECISIONS.md` is explicitly append-only |
| [`fork-sync-policy.md`](./fork-sync-policy.md) | retired at the 2026-07-27 hard fork |
| `fork/patch-ledger.md` | retired with the patch-stack model |

`docs/fork/ideal-base/` is a live program, but its records are appended rather
than rewritten. Do not edit a prior decision to make the program look tidier.

## What is checked automatically

`scripts/check_docs_references.py` enforces three rules across active documents,
skipping the frozen forensic trees above:

- **broken-link**: a Markdown link to a repository-relative path that does not exist.
- **machine-local**: a home-directory path (private notes trees, `/Users/...` style absolute paths) that no other reader can resolve. This is a ratchet against `scripts/docs_references_budget.json`, currently non-zero, and `--update` refuses to raise any file's count.
- **retired-rail**: an instruction to install through any channel the distribution contract retired (Homebrew, AUR, piping a downloaded script into a shell, `cargo install`, TestFlight). Sentences that *prohibit* those rails are not findings. Note that `tests/test_nix_distribution_policy.py` bans several of these tokens outright, with no exemption for prose that merely names them, so this document describes them rather than spelling them.

Run `./scripts/d01_scoreboard.sh` for the current numbers.

The checker is not yet wired into CI, so it prevents nothing on its own today;
wiring it edits a governance-protected workflow and needs the recorded ruleset
maintenance procedure.
