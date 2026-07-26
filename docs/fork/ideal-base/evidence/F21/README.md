# F21 evidence — full deterministic CI/package/updater gate, twice from clean state

**Node:** F21 (verify / deterministic / W3), depends on F17, F18, F19, F20c.
**Branch:** `f20c-retire-distribution`.
**Commit under test:** `2add7453f3a6` (clean tree, verified before and after both runs).

## What F21 actually asks

> All required suites, Nix build, installed assets, and updater matrix pass
> twice at one commit.

The claim is *not* "the suite passes" — F17 already gates that on every PR. F21
asks whether the **packaged artifact and the installed behaviour are
reproducible and self-consistent at a single source identity**. Those are
different questions, and only the second one can catch a package that builds
differently on a second run, or an installed binary whose update behaviour
depends on machine state rather than on the binary itself.

## How determinism is judged

`scripts/f21_integration_gate.py` runs four phases per run and compares runs on
a per-check **fingerprint**, never on raw logs.

Logs were rejected as the comparison basis on purpose: they carry timestamps,
durations, per-run temp paths and test ordering, none of which are part of the
claim. Diffing them would produce noise that has to be waved away by hand, and a
gate whose failures are routinely waved away is not a gate.

A fingerprint is the normalized fact a check establishes:

| phase | fingerprint | why that is the right claim |
|---|---|---|
| `suites` | `passed=N failed=0` | Pinning the **count**, not the exit code. A suite that silently stops running half its tests between runs still exits 0; that drift is exactly what this gate exists to catch. |
| `package` | the nix store path | The store path *is* the reproducibility claim: same source → same derivation → same output hash. Nothing weaker proves it. |
| `install` | asset set, version string | F18's launch gate and F19's share-path assets, read off the real store output. |
| `updater` | decline verdict, doctor origin | F20a/F20c, exercised against the **installed** binary under an isolated `JCODE_HOME`. |
| `residue` | new home entries, session delta | The gate must not dirty the machine it measures. |

## Result: 12/12 checks, both runs, every fingerprint identical

| check | run 1 | run 2 | agree |
|---|---|---|---|
| `suites.jcode-base` | PASS `passed=1205 failed=0` | PASS `passed=1205 failed=0` | yes |
| `suites.jcode-tui` | PASS `passed=1867 failed=0` | PASS `passed=1867 failed=0` | yes |
| `suites.jcode-app-core` | PASS `passed=1136 failed=0` | PASS `passed=1136 failed=0` | yes |
| `package.nix_build` | PASS `fa0mbkdylvqnr3r66dx9if4m743y01d9-jcode-0.46.0` | PASS `fa0mbkdylvqnr3r66dx9if4m743y01d9-jcode-0.46.0` | yes |
| `install.assets` | PASS `present=bin/jcode,share/jcode/web/jcode-mobile` | PASS same | yes |
| `install.mobile_entrypoint` | PASS `index.html=yes` | PASS `index.html=yes` | yes |
| `install.launches` | PASS `jcode v0.46.0 (2add745)` | PASS `jcode v0.46.0 (2add745)` | yes |
| `updater.declines_self_update` | PASS `declined=True downloaded=False` | PASS same | yes |
| `updater.no_retired_layout_written` | PASS `builds_dir=absent` | PASS `builds_dir=absent` | yes |
| `updater.doctor_origin` | PASS `origin=nix` | PASS `origin=nix` | yes |
| `residue.real_home_untouched` | PASS `added=none` | PASS `added=none` | yes |
| `residue.no_leaked_sessions` | PASS `session_delta=0` | PASS `session_delta=0` | yes |

Run 1 took ~636s (cold nix build, 253s of it); run 2 took ~189s, hitting the
store for an identical derivation. **That timing gap is itself the
reproducibility evidence**: the second build produced the same output path
without rebuilding.

Full manifest: `two-run-manifest.md`. Machine-readable: `two-run.json`.

## Proving the gate is not vacuous

A gate that cannot fail proves nothing, so it was attacked in three ways.

**1. The comparator.** `--self-test` asserts identical runs agree, diverging
fingerprints are caught, a missing check is caught, a single run cannot prove
determinism, and — the case a naive exit-code-only gate would wave through —
**two runs that both pass but disagree must fail**. Sabotaging `compare()` to
return "agree" turns three of those red.

**2. The checks themselves.** The install/updater checks were run against a
source-built binary presented as if it were the package. Four of six correctly
refused to certify it:

```
[FAIL] install.assets: present=bin/jcode          <- no share/ assets
[FAIL] install.mobile_entrypoint: index.html=no
[PASS] install.launches: jcode v0.46.0-dev (813fe24dc, dirty)
[FAIL] updater.declines_self_update: declined=False
[PASS] updater.no_retired_layout_written: builds_dir=absent
[FAIL] updater.doctor_origin: origin=source       <- not nix
```

The two that pass are honest: a source binary *does* launch, and it *didn't*
write a retired layout. They are not discriminating checks on their own, which
is why they are not the whole gate.

**3. Residue.** Sabotaging both residue checks to `ok=True` turns the
corresponding self-test assertions red. The self-test also pins that *removal*
is not reported as residue.

## Assumptions that were wrong, and were checked before use

Three things this harness initially assumed turned out to be false. Each was
caught by reading the source rather than trusting a plausible-sounding name.

1. **`JCODE_NO_NETWORK` does not exist.** Invented from memory. It would have
   set a variable nothing reads, giving a false sense of offline isolation.
   Replaced with `JCODE_NON_INTERACTIVE`, which is real.
2. **The accepted `doctor` origin set contained two invented values**
   (`external`, `externally-managed`). `Origin` is `#[serde(rename_all =
   "lowercase")]` over `Nix | Published | Retired | Source | Unknown`, so the
   only correct value is `nix`. The looser set would have accepted nothing extra
   in practice but encoded a wrong belief about the API.
3. **`JCODE_NIX_MANAGED` is deliberately *not* set** by the updater phase. It is
   an explicit override, and setting it would force the very answer the check is
   trying to verify. F20a's real claim is that a store-resident binary
   self-declares managed *purely from where it lives*, so the only honest test
   leaves the override absent.

A fourth hole was found by using the gate: it re-read the commit after both runs
but not the **dirty state**, so editing a tracked file mid-gate left `HEAD`
unchanged while the runs no longer shared one source. It surfaced only as a
`-dirty` suffix buried in an install fingerprint. Now both are checked, and the
run above is clean at both ends.

## A real regression found while getting here

F28 restored parallel test execution, verified over three green rounds — on
macOS. Linux CI then failed
`build_resume_command_uses_imported_jcode_session_for_codex` with
`left: Some("jcode_tui-c64dcb6353e032f7")`: binary resolution had fallen through
to `current_exe()`, so `JCODE_HOME` was not the fixture's temp dir.

Root cause was **drop order**. Tuple fields drop in declaration order, so a
fixture returning `(lease, temp, env_guard)` releases exclusion *first* and
restores `JCODE_HOME` *after* — a window in which it still writes the
environment while another test already holds the lease. Verified in a scratch
crate rather than from memory. `isolated_launcher_env` had the identical defect
independently, and additionally clobbers `HOME`.

Both are fixed, and the class is now structural rather than tribal:
`scripts/check_env_lease_drop_order.py` runs in Quality Guardrails and, on the
unfixed tree, reports both real bugs and nothing else.

That fix also exposed a blind spot in an existing gate:
`check_config_env_lease.py` matched only `set_var`/`remove_var`, while the
idiomatic mutator in this repo is `EnvVarGuard::set` — leaving 67 call sites in
`jcode-tui` alone unexamined. Widened accordingly.

## Reproducing

```
scripts/f21_integration_gate.py --self-test   # verify the harness first
scripts/f21_integration_gate.py --runs 2
```

The gate refuses to start on a dirty tree, because two runs over a moving source
cannot prove anything about one commit.

## Artifacts

- `two-run-manifest.md` — the rendered two-run manifest (self-derived).
- `two-run.json` — machine-readable per-check results and durations.
