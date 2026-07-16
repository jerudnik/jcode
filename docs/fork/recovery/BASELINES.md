# Recovery baselines

Append a dated baseline at the start of every recovery phase that depends on refreshed refs. Do not rewrite older snapshots.

## 2026-07-15 pre-scaffold snapshot

| Item | Value |
|---|---|
| Last code commit before recovery docs | `3d80eaf343e690aaa8b428d0b3ed6de64b7464d0` |
| Upstream ref | `upstream/master` |
| Upstream commit | `802f6909825809e882d9c2d575b7e478dce57d3b` |
| Merge base | `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Fork-only commits before scaffold | 286 |
| Upstream-only commits | 246 |
| Fork-changed files since merge base | 927 |
| Upstream-changed files since merge base | 425 |
| Files changed on both sides | 406 |
| Curated sync | `b3ed82a6b`, one parent |
| Visibility ref | `vendor/upstream` at the merge base |

Reproduction:

```bash
up=upstream/master
base=$(git merge-base HEAD "$up")
git rev-list --left-right --count HEAD..."$up"
git show -s --format='%H%n%P%n%aI%n%s' b3ed82a6b
python3 - <<'PY'
import subprocess
up = 'upstream/master'
base = subprocess.check_output(['git', 'merge-base', 'HEAD', up], text=True).strip()
def names(rev):
    out = subprocess.check_output(['git', 'diff', '--name-only', f'{base}..{rev}'], text=True)
    return set(out.splitlines())
fork, upstream = names('HEAD'), names(up)
print(len(fork), len(upstream), len(fork & upstream))
PY
```

The recovery scaffold itself adds documentation commits after this code snapshot. The next session must append its own refreshed baseline rather than editing this one.

## 2026-07-15 Phase 0 refreshed snapshot

Measured at `2026-07-15T06:44:58Z` after fetching `origin` and `upstream`
without pruning. The fetch ran from `2026-07-15T06:44:01Z` through
`2026-07-15T06:44:02Z`; neither tracked tip moved.

| Item | Value |
|---|---|
| Recovery branch | `recovery/2026-07-15` |
| Recovery HEAD | `6ca1fcf2ec2366c7abc99664a485c40d60cec80e` |
| Last code commit before recovery docs | `3d80eaf343e690aaa8b428d0b3ed6de64b7464d0` |
| `origin/main` before and after fetch | `2ccf43fd79c7802b4b6605998e0717f45ef54583` |
| `upstream/master` before and after fetch | `802f6909825809e882d9c2d575b7e478dce57d3b` |
| Merge base | `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Fork-only commits including recovery scaffold | 287 |
| Upstream-only commits | 246 |
| Fork-changed files since merge base | 934 |
| Upstream-changed files since merge base | 425 |
| Files changed on both sides | 406 |
| Fork-only changed files | 528 |
| Upstream-only changed files | 19 |
| Curated sync | `b3ed82a6bc84656518a165d48bfd8253303286a3`, one parent: `8ed75637accdd40ded1f1d3ac8ce1390459b8d1f` |
| `vendor/upstream` | `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`, still the merge base |
| `distro/nix` | `e601b95b299c5a0941864f6480beaf61ba3eefe1` |
| `follow-upstream` | `7195b6a313a3ed57be7810588ff6f14cc97670b9` |

The overlap remains concentrated in runtime code: 323 files under `crates/`,
23 under `scripts/`, 17 under `src/`, 12 under `changelog/`, and smaller
shared surfaces elsewhere. The fork-side count includes 199 `.rerere-cache`
paths and 98 documentation paths; those inflate file totals but do not reduce
the significance of the 323 shared `crates/` paths.

### Preserved local and runtime state

- The branch was created from current `main` without moving `main`.
- The worktree had one pre-existing modification:
  `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`. Its initial diff SHA-256 was
  `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`.
- Four stashes were preserved unchanged:
  - `2026-07-14T14:24:19-04:00`: `WIP fix-config-hotpath-spam part 3 (scorpion): account_failover hot path`
  - `2026-07-14T14:24:01-04:00`: `WIP fix-config-hotpath-spam part 2 (scorpion): Config::load->config() TUI callers`
  - `2026-07-14T14:23:35-04:00`: `WIP fix-config-hotpath-spam (scorpion, stopped mid-work): config warn-once + sidecar log dedup`
  - `2026-07-14T02:15:50-04:00`: `wip before upstream sync`
- Running client: `jcode v0.46.0-dev (65cfde463)`.
- Selfdev `current`: `3d80eaf34`; shared server: `65cfde463`; stable: none.
- A pre-existing pending canary activation named `test-reload-hash` was
  recorded but not changed.

### Known quality-gate baseline

The following commands were run without `--update`; no ratchet was
rebaselined.

| Gate | Exit | Trust status at this checkpoint |
|---|---:|---|
| `scripts/check_code_size_budget.py` | 1 | Trustworthy structural signal; production files exceeded the recorded ratchet. |
| `scripts/check_test_size_budget.py` | 1 | Trustworthy structural signal; test files exceeded the recorded ratchet. |
| `scripts/check_panic_budget.py` | 1 | Verdict quarantined pending parser audit; its `#[cfg(test)]` exclusion uses approximate brace counting. |
| `scripts/check_swallowed_error_budget.py` | 1 | Verdict quarantined pending parser audit; it shares the approximate classifier. |
| `scripts/check_wildcard_reexport_budget.py` | 0 | Passed, total 16. |
| `scripts/check_warning_budget.sh` | 0 | Passed, current 0 and baseline 0. |
| `git diff --check` | 0 | Passed. |
| `bash -n scripts/*.sh` | 0 | Passed. |

The external strategic synthesis used to seed this audit is not authoritative,
but its location and identity are preserved for provenance:
`/tmp/jcode-strategy-final.md`, SHA-256
`6533d5a2208f9efc82b732617581471858e778867c538cafae89f98c13f36e7a`.
Relevant incident records also exist under the untracked source directory
`/Users/jrudnik/notes/projects/jcode/maintenance/`; each seam that relies on
one must record that note's absolute path, content hash, and a concise
source-backed summary in its ledger.

### Reproduction

```bash
git fetch origin
git fetch upstream

head=$(git rev-parse HEAD)
up=$(git rev-parse upstream/master)
base=$(git merge-base "$head" "$up")
git rev-list --left-right --count "$head"..."$up"
git show -s --format='%H%n%P%n%aI%n%cI%n%s' b3ed82a6b
git rev-parse origin/main upstream/master vendor/upstream distro/nix follow-upstream
git stash list --date=iso-strict

python3 - <<'PY'
import subprocess

def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()

head = git("rev-parse", "HEAD")
up = git("rev-parse", "upstream/master")
base = git("merge-base", head, up)

def names(rev):
    output = git("diff", "--name-only", f"{base}..{rev}")
    return set(output.splitlines()) if output else set()

fork = names(head)
upstream = names(up)
print(len(fork), len(upstream), len(fork & upstream))
print(len(fork - upstream), len(upstream - fork))
PY

python3 scripts/check_code_size_budget.py
python3 scripts/check_test_size_budget.py
python3 scripts/check_panic_budget.py
python3 scripts/check_swallowed_error_budget.py
python3 scripts/check_wildcard_reexport_budget.py
bash scripts/check_warning_budget.sh
git diff --check
bash -n scripts/*.sh
```

## 2026-07-15 Phase 0 truth-gate checkpoint

This append records the final Phase 0 state before responsibility mapping. The
snapshot head is `f9c70d1be5e6cb32a46fbd86d9cb62f4e3603c4f`. Later documentation commits make
HEAD differ without changing the measured Rust source or gate counts.

### Integrated quality-gate repair

| Integrated commit | Isolated source | Purpose |
|---|---|---|
| `fb1168a6a` | `c3c3dd760` | Replace both duplicated Rust production/test classifiers with one shared implementation and 12 initial adversarial tests. |
| `0508e3f7b` | `0bcb7ca49` | Cover additional direct test-only item forms, leading file-level `cfg(test)`, five more tests, and the parser-semantic panic baseline correction. |
| `0674fe53d` | `2456111b5` | Tighten the independently stale swallowed-error ratchet by one at the original baseline. |
| `f9c70d1be` | `c53022f4d` | Run all 17 classifier tests before the panic and swallowed-error gates in both quality workflows. |

The final independent review sequence is preserved under
[`reviews/`](./reviews/). Opus first approved the core parser with conservative
limits, then requested a split because the swallowed-error correction was stale
baseline state rather than a parser effect. The bounded re-review approved the
split with no critical or important finding and confirmed byte-identical final
content.

### Final gate interpretation

| Gate | Result at snapshot | Interpretation |
|---|---|---|
| Warning budget | pass, `0` against baseline `0` | trusted green |
| Wildcard re-export budget | pass, total `16` | trusted green |
| Dependency boundaries | pass through the pinned dev shell | trusted green |
| Production size | fail, 60 violations and +6,604 net LOC across the violating files | real structural debt |
| Test size | fail, 31 violations and +3,679 net LOC across the violating files | real structural debt |
| Panic-prone usage | fail, corrected baseline `31` to current `46` | real drift of 15 after removing parser false positives |
| Swallowed-error-like usage | fail, tightened baseline `2,987` to current `3,077` | real drift of 90 after correcting an already-stale allowance |

No gate was run with `--update`. Exact semantics, historical slices, commands,
and conservative parser limits are in [`QUALITY_GATES.md`](./QUALITY_GATES.md).

### Preserved branch topology

The following snapshot uses `recovery/2026-07-15` at `f9c70d1be...`. Counts are
`recovery-only branch-only` from
`git rev-list --left-right --count recovery/2026-07-15...<branch>`.

| Branch | SHA | Counts | Relation |
|---|---|---:|---|
| `main` | `6ca1fcf2ec2366c7abc99664a485c40d60cec80e` | `6 0` | ancestor |
| `backup/pre-stabilization-2026-07-14` | `2ccf43fd79c7802b4b6605998e0717f45ef54583` | `11 0` | ancestor |
| `sync/upstream-v0.46` | `0ad2278ab913eb1c8cbd31bd5fdda54e7ef0b0a7` | `26 0` | ancestor |
| `feat/nix-managed-mode` | `47f848494f51c4d3ef85a0ae7a287e0c5252b2ed` | `89 0` | ancestor |
| `fix/mcp-selfspawn-supervision-hardening` | `e0a8de8e8a34c8f31f3aeb0188661bf9d49c3752` | `95 0` | ancestor |
| `distro/nix` | `e601b95b299c5a0941864f6480beaf61ba3eefe1` | `273 0` | ancestor |
| `follow-upstream` | `7195b6a313a3ed57be7810588ff6f14cc97670b9` | `110 0` | ancestor |
| `agent/hotpath-stabilization` | `e02f40c91e3759a048a4b3a0df109b023d92a7ee` | `11 2` | diverged, preserved worktree |
| `agent/marker-hardening` | `6fc5623a540e6675841481c3faf90b3e8e2fcfbc` | `11 1` | diverged, preserved worktree |
| `orch/f5-name-resolution` | `18f9fa1b88b39fe46760992d060524bef4552b6e` | `461 138` | diverged |
| `orch/failure-scoreboard` | `5e802effebed623c845b29150605800882ecaae1` | `461 138` | diverged |
| `orch/w1-control-log` | `9bd1c4fc0ca8f3515b2ffd723b0a97990d5a3978` | `461 146` | diverged |
| `orch/w3-lifecycle` | `ed88e1bde7a7b24d3fc659c84554ab2db8785b58` | `461 144` | diverged |
| `recovery/orchestrator-s4-20260715` | `6ca1fcf2ec2366c7abc99664a485c40d60cec80e` | `6 0` | ancestor, restored worktree |
| `recovery/orchestrator-s5-20260715` | `6ca1fcf2ec2366c7abc99664a485c40d60cec80e` | `6 0` | ancestor, restored worktree |
| `recovery/orchestrator-s6-20260715` | `6ca1fcf2ec2366c7abc99664a485c40d60cec80e` | `6 0` | ancestor, restored worktree |
| `recovery/fix-gate-parser-2026-07-15` | `c53022f4d4135b43fc86337c9c689a9e73c27807` | `5 4` | diverged isolated source branch |

`vendor/upstream` remains
`631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`, the refreshed merge base and not a
current upstream mirror.

### Preservation incident and repair

Stopping stale legacy worker sessions reclaimed three clean orchestrator
worktrees and their branch refs. Their exact paths, names, and shared SHA had
already been recorded, so the coordinator recreated them non-destructively at
`6ca1fcf2...`:

- `/Users/jrudnik/labs/jcode-orchestrator-s4` on
  `recovery/orchestrator-s4-20260715`.
- `/Users/jrudnik/labs/jcode-orchestrator-s5` on
  `recovery/orchestrator-s5-20260715`.
- `/Users/jrudnik/labs/jcode-orchestrator-s6` on
  `recovery/orchestrator-s6-20260715`.

No unique commit or dirty change was lost. Final preservation checks found eight
registered worktrees, four stashes, and only the pre-existing prompt edit in the
coordinator worktree. Its diff SHA-256 remains
`8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`.

### Successful validation

```text
python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'
# 17 tests, OK

python3 -m py_compile scripts/rust_production_filter.py \
  scripts/check_panic_budget.py scripts/check_swallowed_error_budget.py \
  tests/test_rust_production_filter.py

ruby -e 'require "yaml"; ARGV.each { |p| YAML.load_file(p) }' \
  .github/workflows/ci.yml .github/workflows/fork-ci.yml

nix develop -c bash -c \
  '/Library/Developer/CommandLineTools/usr/bin/python3 \
   scripts/check_dependency_boundaries.py && cargo fmt --all -- --check'
# dependency boundary check passed; rustfmt check passed
```

An archive replay at `f67e7b45...`, overlaid with the integrated scripts and
corrected JSON, passed exactly at panic `31` and swallowed-error `2,987`.

## 2026-07-15 G4 bounded-pilot snapshot

Measured by the checked-in sequential driver at source HEAD `505cd86726f86dc0eedaf3998afae6ed83290d5d` without `--update`.

| Item | Result |
|---|---|
| Exact pilot fixture | 1 selected, 1 passed |
| Classifier | 17/17 passed |
| Panic-prone budget | expected/actual `1/1`, current 46 |
| Swallowed-error-like budget | expected/actual `1/1`, current 3,074 |
| Production-size ratchet | expected/actual `1/1`, 61 findings |
| Test-size ratchet | expected/actual `1/1`, 31 findings |
| Wildcard re-export budget | expected/actual `0/0`, total 16 |
| Warning budget | expected/actual `0/0`, current 0 |
| Shell syntax | expected/actual `0/0` |
| Diff check | expected/actual `0/0` |
| Pilot observations | exactly 1 |
| Forbidden-output hits | 0 |
| Successful evidence manifest SHA-256 | `b4692dc023075d89fcbe94065d089234fa59bbc5777215082870eb00c3842343` |

Preflight and postflight both recorded branch `recovery/2026-07-15`, sole dirty path `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`, prompt diff SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`, four stashes, `vendor/upstream` at `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`, and no active build process. This snapshot supersedes no earlier count or incident record; it appends the fixed G4 observation.
