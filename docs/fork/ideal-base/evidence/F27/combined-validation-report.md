# F27 combined validation report

Reviewed SHA: `cdf6c869007a9b2e5bfc84c872f8c3db5de6e53d`

## Requested cheap checks

| Command | Exit | Result |
| --- | ---: | --- |
| `python3 scripts/ideal_base_railway.py check` | 0 | `8 roots, 52 child nodes, 60 state records, protected hash intact` |
| `python3 scripts/check_swallowed_error_budget.py` | 0 | Improved from baseline: total `3034 -> 2966` |
| `python3 scripts/check_panic_budget.py` | 0 | `total=56 files=24` |
| `python3 scripts/check_critical_path_budget.py` | 0 | All domains within ceilings; lifecycle swallowed-error has headroom 2 |
| `bash scripts/fork-health.sh` | 2 | Usage-only failure: this script requires exactly one of `--fixture` or `--live` |
| `bash scripts/fork-health.sh --fixture docs/fork/ideal-base/evidence/R07/fixtures/governance-valid.json` | 0 | All local invariants and offline governance comparison passed |

## Supplemental focused checks

| Command/check | Exit | Result |
| --- | ---: | --- |
| Advisory policy at `2026-08-01` under uv Python 3.13 | 0 | Current advisory records valid |
| `test_advisory_policy.py` under uv Python 3.13 | 0 | 35 tests passed |
| `python3 scripts/check_tui_render_lock.py` | 0 | 45 locked render-state tests, 0 unlocked |
| `bash scripts/check_ambient_roots.sh` | 0 | 21 direct sites, all allowlisted with stated reasons |
| Current F24 SBOM generator body plus `nix/verify-provenance-sbom.py` | 0 | 947 components, 947 unique `bom-ref`s; structural verification passed |
| Current `test_nix_distribution_policy.py` under uv Python 3.13 | 0 | 9 tests passed |
| Isolated F30 plant in unlisted `.apm/instructions/retired-channel.md` (`yay -S jcode-git`) | 0 | **Unexpected pass; confirms F30-1 policy coverage gap** |

## Environment limitations

- `/usr/bin/python3` is Python 3.9.6 and cannot import `tomllib`; direct F22/F30 runs under that interpreter exited 1 before executing policy logic. They were rerun successfully with uv-managed Python 3.13.14.
- Nix is not installed on this verifier host (`nix` exit 127), so no Nix realization or flake check was repeated.
- No full Cargo build or test suite was run.
