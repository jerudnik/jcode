# Fork security triage

Last reviewed: 2026-07-18

Triage records for advisories the **fork** ignores in `.cargo/audit.toml`
beyond what upstream documents in `docs/SECURITY_DEPENDENCIES.md`. The
Security workflow fails if an ignore in `.cargo/audit.toml` has no row in
either file, so config and rationale cannot drift apart.

Policy (see `.github/workflows/security.yml`):

| Advisory class | Handling |
|---|---|
| Direct vulnerability (workspace dependency) | Gate fails until fixed or triaged here with a retire condition |
| Reachable runtime transitive | Gate fails until triaged; weekly report re-lists for review |
| Build-time / non-compiled-target transitive | Triaged ignore; weekly report re-lists |
| Unmaintained / unsound warnings | Advisory only, listed in the weekly report issue |

## Fork-triaged advisories

There are currently no fork-only advisory rows. Every advisory ignored by
`.cargo/audit.toml`, including the former fork-local `anyhow`, `memmap2`, and
`quick-xml` entries, is now documented in `docs/SECURITY_DEPENDENCIES.md`.

Add a row here only when the fork ignores an advisory that is not already
covered there. Git history preserves the former duplicate rows.

## Review cadence

The weekly Security report (tracking issue, Mondays) re-runs `cargo audit`
with ignores disabled. When reviewing it:

1. Any triaged advisory with a met retire condition: drop the ignore, bump the
   dependency, delete the row.
2. Any new advisory: classify per the policy table, then either fix or add an
   ignore + row in the same commit.
3. Bump "Last reviewed" above.
