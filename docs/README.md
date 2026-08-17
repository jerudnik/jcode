# Documentation map

The repository has two documentation surfaces:

1. **Current documentation** describes the product, architecture, operations,
   and contributor contracts as they work now.
2. **Open issues** under [`issues/`](./issues/) describe acknowledged defects
   that still need work.

Proposals, project tracking, incident evidence, and historical records belong
in the project notes repository, not under `docs/`. Solved issues and completed
or cancelled proposals are deleted. Git history remains the forensic record.

## Authority

Generated root instructions and the APM primitives that produce them bind agent
behavior. Edit `.apm/instructions/*.instructions.md`, then run `apm compile`.
Generated child instruction files may exist locally but are not reader-facing
documentation and must not be linked from committed docs.

Substantial current surfaces include:

| Area | Documents |
| --- | --- |
| Contribution and release | [`agent-workflows.md`](./agent-workflows.md), [`BRANCHING.md`](./BRANCHING.md), [`NIX.md`](./NIX.md) |
| Providers and auth | `AUTH_CREDENTIAL_SOURCES.md`, `PROVIDER_DOCTOR.md`, `AWS_BEDROCK_PROVIDER.md`, `../OAUTH.md` |
| Sessions and server | `SERVER_ARCHITECTURE.md`, `SERVER_LIFECYCLE_INVARIANTS.md`, `RESUME_BEHAVIOR.md` |
| Memory | `MEMORY_ARCHITECTURE.md`, `MEMORY_BUDGET.md` |
| Swarm | `SWARM_ARCHITECTURE.md`, `SWARM_TASK_GRAPH.md` |
| Tools and hooks | `AGENT_TOOL_INTEGRATION.md`, `HOOKS.md`, `SPAWN_HOOK.md` |
| Telemetry and security | `../TELEMETRY.md`, `SECURITY_DEPENDENCIES.md`, `security/` |
| Platform | `WINDOWS.md`, `WRAPPERS.md`, `NIX.md` |

`architecture/` holds deeper current design records for these surfaces. If a
current document disagrees with code, resolve the defect rather than preserving
both claims.

## Open issues

Every direct Markdown child of `docs/issues/` requires YAML frontmatter with:

- `status`: an active state such as `open`, `partial`, or `blocked`
- `priority`
- `owner`
- `opened`: `YYYY-MM-DD`

Delete an issue when it becomes fixed, closed, or intentionally rejected.

## Automated checks

`scripts/check_docs_references.py` checks tracked Markdown for:

- retired repository documentation surfaces
- missing issue frontmatter or solved issues retained as files
- broken repository-relative links
- machine-local references
- stale source-code paths
- instructions that use retired distribution rails

Machine-local and stale-code-path findings are ratcheted by
`scripts/docs_references_budget.json`; all other findings are immediately fatal.
Run `python3 scripts/check_docs_references.py --update` after removing ratcheted
debt. The update command refuses to raise the baseline.
