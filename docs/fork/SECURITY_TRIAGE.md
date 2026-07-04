# Fork security triage

Last reviewed: 2026-07-04

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

| Advisory | Crate | Class | Triage | Retire condition |
|---|---|---|---|---|
| `RUSTSEC-2026-0190` | `anyhow` | direct, unsoundness | `Error::downcast_mut()` unsoundness; jcode uses `anyhow` broadly but not that pattern intentionally. No patched release exists yet. | Bump `anyhow` as soon as a patched release lands. |
| `RUSTSEC-2026-0186` | `memmap2` | transitive runtime (rendering/embedding) | Unchecked pointer-offset unsoundness via fontdb/usvg/resvg/tract stacks. Not in the auth/provider/network path. | Upstream rendering/embedding stack upgrades remove all affected versions. |
| `RUSTSEC-2026-0195` | `quick-xml` | transitive build-time (Linux desktop only) | Wayland protocol code generation via `wayland-scanner` in the desktop build stack. Never compiled into the fork's macOS/Nix TUI artifact. | Wayland/winit stack accepts `quick-xml >=0.41`. |
| `RUSTSEC-2026-0194` | `quick-xml` | transitive build-time (Linux desktop only) | Same path as RUSTSEC-2026-0195. | Same as above. |

Upstream-triaged ignores (`lettre`, `rustls-webpki` x4, `lopdf`) are documented
in `docs/SECURITY_DEPENDENCIES.md` and inherited unchanged.

## Review cadence

The weekly Security report (tracking issue, Mondays) re-runs `cargo audit`
with ignores disabled. When reviewing it:

1. Any triaged advisory with a met retire condition: drop the ignore, bump the
   dependency, delete the row.
2. Any new advisory: classify per the policy table, then either fix or add an
   ignore + row in the same commit.
3. Bump "Last reviewed" above.
