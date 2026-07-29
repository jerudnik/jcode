# Fork security triage

Last reviewed: 2026-07-29

Fork-specific narrative for advisories the fork ignores in `.cargo/audit.toml`
beyond what `docs/SECURITY_DEPENDENCIES.md` covers.

**This file is no longer an enforcement surface.** Since F22, the
machine-readable ownership record is `docs/security/advisories.toml` and the
gate is `scripts/check_advisory_policy.py`, run from the `advisory ownership
policy` job in `.github/workflows/security.yml` and the `advisory ownership`
gate in `scripts/preflight.sh`. The old check grepped these Markdown files for
the advisory ID, which a passing mention satisfied and which carried no owner
or expiry. Adding prose here suppresses nothing.

Policy (enforced by `scripts/check_advisory_policy.py`):

| Advisory class | Handling |
|---|---|
| Direct vulnerability (workspace dependency) | Gate fails until fixed or given a complete record in `docs/security/advisories.toml` |
| Reachable runtime transitive | Gate fails until triaged; weekly report re-lists for review |
| Build-time / non-compiled-target transitive | Triaged ignore; weekly report re-lists |
| Unmaintained / unsound warnings | Advisory only, listed in the weekly report issue |

## Fork-triaged advisories

There are currently no fork-only advisory rows. Every advisory ignored by
`.cargo/audit.toml`, including the former fork-local `anyhow`, `memmap2`, and
`quick-xml` entries, is now documented in `docs/SECURITY_DEPENDENCIES.md`.
Each also has a structured record in `docs/security/advisories.toml`.

Add a row here only when the fork needs narrative that neither the structured
record nor `docs/SECURITY_DEPENDENCIES.md` carries. Git history preserves the
former duplicate rows.

## Review cadence

The weekly Security report (tracking issue, Mondays) re-runs `cargo audit`
with ignores disabled. When reviewing it:

1. Any triaged advisory with a met `retire_when`: drop the ignore from
   `.cargo/audit.toml` and delete its record. The stale-record check fails if
   you do only one.
2. Any new advisory: classify per the policy table, then either fix it or add
   an ignore plus a complete record in the same commit.
3. Bump "Last reviewed" above.

Expiry enforces the same thing on a deadline: the gate turns red on its own if
nobody reviews.
