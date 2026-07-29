# Dependency Security Triage

Last reviewed: 2026-07-29

## Where the record lives

| File | Role |
|---|---|
| `.cargo/audit.toml` | The advisory IDs `cargo audit` suppresses. Nothing else: cargo-audit validates this file against a closed schema and rejects any extra key, so it cannot hold ownership metadata. |
| `docs/security/advisories.toml` | **The machine-readable ownership record.** One `[[advisory]]` per accepted advisory with `id`, `crate_name`, `owner`, `accepted`, `expires`, `affected_surface`, `rationale`, and `retire_when`. |
| `scripts/check_advisory_policy.py` | The checker that proves the two agree and that no acceptance has expired. |
| `tests/test_advisory_policy.py` | Fixtures that observe the checker failing on each planted violation, so the gate is never trusted unseen. |
| This file | Human-readable narrative: priority order, review cadence, and what changed. It is not the enforcement surface. |

An acceptance that is undocumented, incomplete, stale, or expired fails the
`advisory ownership policy` job in `.github/workflows/security.yml` and the
`advisory ownership` gate in `scripts/preflight.sh`. Neither reads a Markdown
table; adding a row here does not suppress anything, and deleting one does not
break the build.

This is not an allowlist. It is a triage record so advisories are visible and
actionable.

## Enforcement rules

- Every ID in `.cargo/audit.toml` must have a record in
  `docs/security/advisories.toml` — an ignore with no owner fails CI.
- Every record must carry all eight fields, non-blank.
- `expires` is an ISO date. On or after that date, the gate fails: an ignore
  cannot outlive the argument that justified it. Renewing means re-reading the
  advisory, re-checking `retire_when`, and moving `accepted` forward with a
  fresh justification in the same commit — not re-stamping the date.
- `expires` may be at most `[policy].max_expiry_days` (365) past `accepted`.
- A record whose ID is no longer ignored also fails, so retired acceptances get
  deleted rather than accumulating.
- The checker takes the current date from `--today`, then
  `$ADVISORY_POLICY_TODAY`, then the system date. Tests and preflight inject it,
  so expiry behavior never depends on when the suite happens to run.
- Unmaintained/unsound *warnings* are advisory by design. `cargo audit` fails
  only on vulnerabilities; do not add `--deny warnings` semantics.

## Current accepted advisories

Authoritative fields (owner, expiry, retirement condition) live in
`docs/security/advisories.toml`. This table is the narrative summary.

| Advisory | Crate | Dependency path | Affected area in jcode | Triage |
|---|---|---|---|---|
| `RUSTSEC-2026-0141` | `lettre` | `jcode-notify-email -> lettre` | Notification email sending | The hostname-verification defect is in the Boring TLS backend. Jcode enables rustls/native-tls only, so the defective code is not compiled in. |
| `RUSTSEC-2026-0098` | `rustls-webpki` | `rustls` dependency stack | TLS certificate validation | Name constraints for URI names incorrectly accepted. Transitive; fix needs major `aws-sdk`/`imap` bumps. |
| `RUSTSEC-2026-0099` | `rustls-webpki` | `rustls` dependency stack | TLS certificate validation | Name constraints incorrectly accepted for wildcard certificates. Same blocked upgrade path. |
| `RUSTSEC-2026-0104` | `rustls-webpki` | `rustls` dependency stack | TLS CRL parsing | Reachable panic in CRL parsing; jcode loads no CRLs. |
| `RUSTSEC-2026-0049` | `rustls-webpki` | `aws-smithy` rustls 0.21, `imap`/`rustls-connector` rustls 0.22 | TLS CRL handling | CRLs not authoritative by Distribution Point. Fix needs rustls-webpki >=0.103.10, blocked by the pinned older rustls stacks. |
| `RUSTSEC-2026-0187` | `lopdf` | `jcode-pdf -> pdf-extract 0.8.2 -> lopdf 0.34` | PDF text extraction (`/pdf`, PDF reads) | Stack overflow on deeply nested PDF objects; reached only on a user-opened PDF, not in the auth/provider path. `pdf-extract 0.8.2` pins `lopdf 0.34`. |
| `RUSTSEC-2026-0190` | `anyhow` | Workspace-wide | Error handling across CLI, providers, tools, TUI | Unsoundness in `Error::downcast_mut()`; jcode never calls it. No patched release in the lockfile. |
| `RUSTSEC-2026-0186` | `memmap2` | `fontdb`/`usvg`/`resvg`, `tract-onnx`, other rendering/embedding paths | TUI rendering, Mermaid/SVG, embedding | Unsoundness in unchecked pointer offset. Linux CI also sees an older transitive `memmap2` alongside 0.9. |
| `RUSTSEC-2026-0195` / `RUSTSEC-2026-0194` | `quick-xml` | `jcode-desktop -> winit 0.29 -> smithay-client-toolkit 0.18 -> wayland-scanner 0.31` | Linux Wayland protocol codegen in the desktop build stack | Allocation/runtime DoS in XML parsing. `wayland-scanner` is a build-time proc macro over trusted vendored protocol XML. A `quick-xml >=0.41` bump is blocked by its `^0.39` constraint. |

## Advisories fixed rather than accepted

- `RUSTSEC-2026-0217` (`tract-nnef` 0.21.10, integer overflow to out-of-bounds
  read in the NNEF tensor parser) was **fixed, not ignored**, on 2026-07-29.
  The fix required moving `jcode-embedding` from `tract-* 0.21` to `0.22`: the
  patched 0.21 releases (0.21.16, 0.21.17) constrain `time` to `<0.3.42`, while
  `azure_identity 1.0` requires `time ^0.3.47`, so no in-line 0.21 bump
  resolves. `tract-linalg 0.22.x` relaxes the constraint to `^0.3.23`.
- `RUSTSEC-2024-0320` (`yaml-rust`) left the graph on 2026-03-05 by trimming
  `syntect` features to built-in syntax/theme dumps instead of YAML loading.
- `RUSTSEC-2023-0086` (`lexical-core` via `imap-proto`) is **not** in the
  current ignore list and does not appear in the current `cargo audit` output.
  It was listed here until 2026-07-29 without ever being ignored; the row is
  removed rather than carried as a phantom.

## Priority order

1. `rustls-webpki` TLS advisories via the rustls stack
2. `anyhow` once a patched release is available
3. `lopdf` via `pdf-extract`
4. `memmap2` via rendering/embedding transitive stacks
5. `quick-xml` via the desktop Wayland/winit build-time stack
6. `lettre` if jcode ever enables `boring-tls`

## Review cadence

The weekly Security report (tracking issue, Mondays) re-runs `cargo audit` with
ignores disabled. When reviewing it:

1. Any accepted advisory whose `retire_when` is now met: drop the ignore from
   `.cargo/audit.toml`, delete the record from `docs/security/advisories.toml`,
   and remove the row above. The stale-record check fails if you do only one.
2. Any new vulnerability: fix it if a compatible version exists, otherwise add
   an ignore *and* a complete record in the same commit.
3. Bump "Last reviewed" above.

Expiry does the same job on a hard deadline: the gate turns red on its own if
nobody reviews.

## Notes

- Before changing dependency versions, run:
  - `scripts/dev_cargo.sh check`
  - `scripts/dev_cargo.sh test -j 1`
  - `scripts/security_preflight.sh`
  - `python3 scripts/check_advisory_policy.py`
