# F30 — Adversarial verification: Nix-only distribution & native-iOS retirement

- **Verified commit:** `eee5ccc710b45be50fe7b6556c958972b7ae51a4` (`git rev-parse HEAD`, worktree `/private/tmp/w4-f30`, branch `automation/w4-f30`)
- **Role:** verify-only. No production file was changed. Every plant in this report was reverted and the reverted state re-proven green.
- **Verdict:** the landed transition is **substantially real and non-vacuous**, with **four coverage gaps** recorded as bounded fix nodes below. No gap invalidates the transition; all four are guard-strength gaps, not regressions of the shipped behavior.

## Gate results

| Gate | Question | Result |
|---|---|---|
| 1 | Are retired distribution channels actually absent from the active surface? | **PASS** (exhaustive sweep, `logs/active-surface-token-sweep.txt`) |
| 2 | Do packaged `web/jcode-mobile` assets resolve and serve outside a source checkout? | **PASS** (`logs/gate2-packaged-mobile.txt`, `logs/gate2-served-outside-checkout.txt`) |
| 2b | Does `jcode update` on a Nix-managed binary refuse to self-overwrite or recreate the retired layout? | **PASS** (`logs/gate-update-noselfoverwrite.txt`) |
| 3 | Is the policy gate non-vacuous — does it go RED and name the plant? | **PASS with holes** (`logs/plant-transcript.txt`, `logs/flake-check-gate.txt`) |
| 4 | Is the CI-path (flake check) gate equivalent to the raw pytest gate? | **PASS** (`logs/flake-check-gate.txt`, `logs/flake-ios-plant-tracked.txt`) |

The gate under test is `tests/test_nix_distribution_policy.py`, invoked from `RELEASING.md:31` and `scripts/preflight.sh`, and wrapped as the flake check `checks.<system>.nix-distribution-policy`.

> Environment note for reproducers: PATH is stripped in this worktree and `nix shell nixpkgs#python3Packages.pytest` still resolves system python first (`No module named pytest`). Invoke the absolute interpreter, or use `nix build .#checks.aarch64-darwin.nix-distribution-policy`.
>
> Work-graph note: `WORK_GRAPH.json` is keyed under `all_nodes`, not `nodes`; `jq '.nodes[]'` errors.

---

## Gate 1 — exhaustive absence sweep (PASS)

Swept ~35 tokens across the active surface (excluding `docs/fork/**`, `docs/archive/**`, `changelog/**`, binaries, SVG): Homebrew (`brew install`, `brew tap`), AUR (`yay`, `paru`, PKGBUILD, `jcode-bin`), `scripts/install.sh|install.ps1|uninstall.sh|quick-release.sh|update_packages.sh`, `releases/download`, `gh release upload|create`, `actions/upload-artifact`, TestFlight / `xcodebuild` / `JCodeMobile` / `docs/IOS_APP` / `.github/workflows/ios*`, `cargo publish`, winget / scoop / choco.

**Every hit on the active surface is either the policy test's own token constants or unrelated.** `git ls-files` confirms no `ios/`, `install.sh`, `install.ps1`, `uninstall*`, `IOS_APP*`, or `testflight*` files exist. No Swift, `.xcodeproj`, `Info.plist`, or `codesign` residue outside archived proposals.

Unrelated hits triaged in `logs/nonmd-hit-triage.txt`: `brew install brightness` (a computer-use tool's error message), `brew install git` and rustup's `curl | sh` (selfdev toolchain bootstrap guidance), rustup in `build_linux_compat.sh`. None advertise a jcode distribution channel.

`.github/workflows/release.yml` is genuinely metadata-only: it refuses to publish over a release with assets and re-checks asset count after publishing, failing if non-zero.

## Gate 2 — packaged mobile assets outside a checkout (PASS)

```
nix build .#packages.aarch64-darwin.jcode
  → /nix/store/j4m1y602ggih17mp9x19a5ccndvw54k5-jcode-0.46.0
```

1. `share/jcode/web/jcode-mobile/*` is **byte-identical** to the repo's `web/jcode-mobile/` (sha256 diff clean).
2. Ran the store binary from a fresh `mktemp -d` sandbox with isolated `HOME`/`JCODE_HOME`, **no `.git`, no `web/`**:
   - `jcode mobile-server start --port 8793 --bind 127.0.0.1` → `pid 98265`
   - `jcode mobile-server status` → `web root: /nix/store/j4m1y…/share/jcode/web/jcode-mobile` (resolved to the **store**, not a checkout)
   - `GET /` → `200`, 503 bytes, sha256 `8b6fb426…` == store `index.html` sha256
   - `app.js` (57478 B), `style.css` (17104 B), `surface_state.mjs` (11227 B), `surface_commands.mjs` (9856 B), `surface_workspace_store.mjs` (18036 B) — **all served bytes == store bytes**
   - `stop` → clean shutdown; post-stop `GET /` → connection refused
3. `jcode doctor --json` → `client.origin = nix`, `client.path = /nix/store/j4m1y…/bin/jcode`

Corrects an earlier probe error: `jcode mobile-server` is a subcommand group (`start`/`status`/`logs`/`stop`/`open`), not a flag-only server; `--port` alone is invalid and binds nothing.

## Gate 2b — update path does not self-overwrite (PASS)

`jcode update` from the store binary prints Nix guidance only (Home Manager / `nix profile upgrade` / `nix flake update`) and exits 0. Store binary is unchanged (`-r-xr-xr-x root nixbld`, immutable). No `$JCODE_HOME/builds` or `$JCODE_HOME/current` retired layout was created. The only files written were a log and a prune stamp — **no download occurred**.

## Gate 3 — non-vacuity (PASS with holes)

Baseline clean = `9 passed, 11854 subtests`. Eleven plants, all reverted; full transcript in `logs/plant-transcript.txt`.

**Caught (went RED, naming the plant):**

| Plant | Signal |
|---|---|
| `brew install jcode` in `README.md` | `SUBFAILED(path='README.md', token='brew install jcode')` |
| restored `scripts/install.sh` | `retired path returned: scripts/install.sh` |
| restored `ios/App.swift` (tracked) | `retired native-iOS path restored: ios` |
| `gh release upload` in `nix.yml` | RED |
| `JCodeMobile` in `crates/jcode-app-core/src/update.rs` | RED |
| `releases/download` in `crates/jcode-app-core/src/update.rs` | RED |

The flake check reproduces the raw-pytest verdict exactly (`logs/flake-check-gate.txt`).

**Escapes (stayed GREEN) — real coverage holes:**

- **PLANT J** — `` `jcode update installs` the newest release binary`` in `docs/agent-workflows.md`. The token *is* in `FORBIDDEN_ACTIVE_DOC_TEXT`, but the file is not in `ACTIVE_DISTRIBUTION_DOCS`, so it is never scanned.
- **PLANT K** — TestFlight/iOS advertisement in `.apm/instructions/main.instructions.md`. A naive append was caught only *incidentally* by `check_agent_instructions.py`'s 8192-byte prompt budget. A **byte-neutral** plant (replacing the retirement sentence with `brew install jcode` + TestFlight text, 187→163 bytes) passed **both** `check_agent_instructions.py` and the policy gate → genuine escape (`logs/apm-instruction-plant.txt`).
- `brew install jcode` in `CONTRIBUTING.md` — not allowlisted.
- AUR `yay -S jcode-git` in `docs/NIX.md` — allowlisted file, but AUR tokens are absent from `FORBIDDEN_ACTIVE_DOC_TEXT` entirely.
- curl-installer phrasing in `README.md` — no `curl … | sh` token in the forbidden list.

`logs/escape-crosscheck.txt` confirms no other deterministic gate catches these: `check_agent_instructions.py` only enforces a byte budget, `scripts/f20c_removal_report.sh` does not scan doc prose, and `docs_impact_advisory.py` is advisory/non-blocking and requires `--base/--head`.

**Root cause of all escapes:** the allowlist is an 8-file **opt-in**. `logs/allowlist-coverage-gap.txt` enumerates **39** active markdown files that discuss install/update/distribution; **8 are guarded, 31 are not**. A retired-channel claim can be reintroduced silently in any of the 31.

## Gate 4 — flake-check semantics (PASS, with a documented caveat)

An **untracked** `ios/App.swift` did *not* fail the flake check; the same file **tracked** (`git add`) failed with `retired native-iOS path restored: ios` (`logs/flake-ios-plant-tracked.txt`). This is standard git-flake source semantics (untracked files are excluded from the flake source), not a policy defect — CI only ever evaluates tracked content. Recorded so a future auditor does not mistake it for a hole.

## Additional findings — stale residue (`logs/stale-residue-refs.txt`)

- **R1:** `scripts/lib/configure_path.sh` documents that it "is kept in sync with the inline copy in **install.sh**, which must stay self-contained because it is run via `curl ... | bash`." `install.sh` no longer exists, and a repo-wide search finds **zero callers** of `configure_path.sh`. It is an orphan whose comment advertises a retired channel.
- **R2:** `crates/jcode-build-support/src/paths.rs:1079` references `uninstall.sh` alongside `jcode doctor --clean-retired-layout`. The doctor flag is real (`src/cli/args.rs:246`, `src/cli/commands/doctor.rs:204`); `uninstall.sh` is retired. Doc-comment only, no behavioral impact.
- **Workflow lint-list gap:** `ci.yml`, `freebsd-smoke.yml`, and `governance-root.yml` are absent from the flake's `checkSrc` source filter **and** from both actionlint lists (`.github/workflows/nix.yml`, and the `workflow-syntax` check in `flake.nix`), while `distributionPolicySrc` includes the whole `.github/workflows` directory. Separately, `scripts/fork-health.sh` requires every workflow to appear in the `docs/BRANCHING.md` CI table.

---

## Proposed fix nodes (bounded; **not** applied by F30)

### F30-FIX-1 — close the allowlist opt-in hole
- **Contract:** a retired-channel claim in any active doc fails the policy gate.
- **Owned paths:** `tests/test_nix_distribution_policy.py`
- **Change:** either (a) extend `ACTIVE_DISTRIBUTION_DOCS` to include `CONTRIBUTING.md`, `docs/agent-workflows.md`, and `.apm/instructions/**`; or preferably (b) invert to an **opt-out** model — scan all tracked `*.md` plus `.apm/instructions/**` outside `docs/fork/**`, `docs/archive/**`, `changelog/**`, with a small explicit exception list.
- **Gates:** plant PLANT J and PLANT K, assert RED naming the file; revert; assert `9 passed`.

### F30-FIX-2 — widen the forbidden-token list
- **Contract:** AUR and curl-pipe installers are as forbidden as Homebrew.
- **Owned paths:** `tests/test_nix_distribution_policy.py`
- **Change:** add `yay -S jcode`, `paru -S jcode`, `PKGBUILD`, `aur.archlinux.org`, and a curl-pipe pattern (`curl … | sh`/`| bash` referencing a jcode install URL) to `FORBIDDEN_ACTIVE_DOC_TEXT`. Must not trip the legitimate rustup bootstrap strings in `scripts/build_linux_compat.sh` and `crates/jcode-app-core/src/tool/selfdev/setup.rs` — scope the curl pattern to jcode-hosted URLs.
- **Gates:** plant `yay -S jcode-git` in `docs/NIX.md` → RED; confirm `scripts/build_linux_compat.sh` still passes clean.

### F30-FIX-3 — workflow lint-list completeness
- **Contract:** every workflow under `.github/workflows` is actionlint-checked, or its exclusion is deliberate and documented.
- **Owned paths:** `flake.nix`, `.github/workflows/nix.yml`
- **Change:** add `ci.yml`, `freebsd-smoke.yml`, `governance-root.yml` to `checkSrc` and both actionlint lists — or replace the hand-maintained lists with a directory glob so the set cannot drift.
- **Gates:** `nix build .#checks.<system>.workflow-syntax` with a deliberate syntax error planted in `ci.yml` must go RED.

### F30-FIX-4 — retire orphaned installer residue
- **Contract:** no active file documents or supports a retired distribution channel.
- **Owned paths:** `scripts/lib/configure_path.sh`, `crates/jcode-build-support/src/paths.rs`
- **Change:** delete `scripts/lib/configure_path.sh` (zero callers) and add it to `RETIRED_PATHS`; drop the `uninstall.sh` reference from the `paths.rs` doc comment.
- **Gates:** repo-wide search for `configure_path` returns only the policy test; `9 passed` after the `RETIRED_PATHS` addition.

---

## Logs

| File | Contents |
|---|---|
| `logs/active-surface-token-sweep.txt` | Gate 1 exhaustive ~35-token sweep |
| `logs/plant-transcript.txt` | Gate 3, all 11 plants with RED/GREEN outcomes |
| `logs/apm-instruction-plant.txt` | Byte-neutral `.apm` plant proving a genuine escape |
| `logs/escape-crosscheck.txt` | Proof no other deterministic gate catches the escapes |
| `logs/allowlist-coverage-gap.txt` | 39 install-discussing docs vs 8 allowlisted |
| `logs/nonmd-hit-triage.txt` | Triage of non-markdown token hits |
| `logs/stale-residue-refs.txt` | R1/R2 orphaned installer residue |
| `logs/gate2-packaged-mobile.txt` | Store build + byte-identity + out-of-checkout launch |
| `logs/gate2-served-outside-checkout.txt` | HTTP serve proof, served bytes == store bytes |
| `logs/gate-update-noselfoverwrite.txt` | `jcode update` is guidance-only |
| `logs/flake-check-gate.txt` | Flake-check plant/revert cycle |
| `logs/flake-ios-plant-tracked.txt` | Tracked-vs-untracked flake source semantics |

`SHA256SUMS` covers every file in this directory.
