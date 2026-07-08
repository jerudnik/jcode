# FORK HARDENING — GREEN SCOREBOARD (ground-truth referee, no priors)

Group: GREEN. Date: 2026-07-08. HEAD `75b20b215`. Vendor ref
`github/vendor/upstream` = `631935dd1`. Every row has a reproducible command.
Verdict legend: **HELD** (model-accurate) / **DRIFT-OK** (drifted tolerably) /
**DRIFT-BAD** (drifted dangerously) / **BROKEN** (invariant failing now).

## Scoreboard

| # | Metric | Model claim | Measured (2026-07-08) | Command | Verdict |
|---|---|---|---|---|---|
| 1 | commits over vendor | "30 mostly-additive" | **180** | `git rev-list --count github/vendor/upstream..HEAD` | DRIFT-OK (count ≠ conflict surface) |
| 2 | files touched vs vendor | 107 (47 new / 60 mod) | **500** (311 A / 186 M / 3 D) | `git diff --name-status github/vendor/upstream HEAD` | DRIFT-OK |
| 3 | source files deleting >5 lines | **7** | **16** non-test .rs (13 incl. some tests in baseline) | `git diff --numstat … -- '*.rs' \| awk '$2>5' \| grep -vi test` | DRIFT-OK (still small) |
| 4 | total upstream .rs lines deleted | (implicit "small") | **581** | `git diff --numstat … -- '*.rs' \| awk '{d+=$2}END{print d}'` | HELD (small) |
| 5 | total .rs lines added | (implicit "most") | **17,241** | same, `{a+=$1}` | HELD |
| 6 | **add : delete ratio** | "additions dominate" | **≈30 : 1** | rows 4+5 | **HELD (strongly)** |
| 7 | .rs files with ZERO deletions | "most are additive" | **113 of 186** | `… \| awk '$2==0' \| wc -l` | HELD |
| 8 | skill.rs "converted additive 20→0" | 0 deletions | **12 deletions** | `git diff --numstat … -- …/skill.rs` | DRIFT-BAD (claim false; but is a `.apm` *extension*, not rot — see note) |
| 9 | agent.rs "additive-only" | additive-only | **19 deletions** (compaction-dispatch gating) | `git diff --numstat … -- …/agent.rs` | DRIFT-BAD (claim false; real behavior edit) |
| 10 | swarm/comm files under ledger | (should be) | **0 rows** for swarm/comm/compaction/session_search | `grep -c … docs/fork/patch-ledger.md` | DRIFT-BAD |
| 11 | rerere cache | "resolve once, ever" | **100 entries, 199 tracked files, healthy, no GC** | `ls .rerere-cache/ \| wc -l` | HELD (functional); DRIFT-OK (unbounded) |
| 12 | `jcode doctor` client+server+built-from | shipped (NS4) | **present, all 3 lines emit** | `jcode doctor` | HELD |
| 13 | 6h CI rebase rail | shipped | **`sync.yml` cron `17 */6 * * *`, force-with-lease, rerere import, abort-on-new** | `.github/workflows/sync.yml` | HELD |
| 14 | vendor/distro refs moving | (alive) | vendor `631935dd1` 2026-07-07; distro `e601b95b2` 2026-07-04 | `git log -1 --date=short github/vendor/upstream github/distro/nix` | HELD |
| 15 | fork-health check 5 (no workflow diff main vs distro) | invariant | **FAILS: `sync.yml` differs** (`1f5cc7aac`) | `git diff --name-only github/distro/nix github/main -- .github/workflows/` | **BROKEN** |
| 16 | model doc cites correct sync file | "`nix.yml`" | sync extracted to **`sync.yml`** | grep workflows | DRIFT-OK (doc-only) |
| 17 | patch-ledger freshness | living index | 85 lines, last `2026-07-04`; ~half invasive files unlisted | `git log -1 --date=short -- docs/fork/patch-ledger.md` | DRIFT-BAD |
| 18 | fork gate green | (should pass) | **exit 0** (186 fork-touched .rs, fmt clean) | `scripts/fork-touched-clippy.sh --fmt` | HELD |

## Referee adjudication of RED vs BLUE

- **RED#1 (numbers stale 6–20×):** UPHELD as fact (rows 1–3), but the axis that
  matters (rows 4–7) shows the *conclusion* is intact. BLUE wins the interpretation;
  RED wins the "recalibrate the doc" demand. Both correct.
- **RED#2 (invasive swarm/comm subsystem):** the files ARE upstream-edits (verified),
  so "edits not new files" is TRUE. But severity is overstated: 8 files, ~120
  deletions inside ~1,900 additions (row 6 ratio locally holds). Verdict: real
  governance gap (row 10), NOT a critical invasiveness problem. BLUE wins severity.
- **RED#3 (skill.rs regressed):** the "0 deletions" claim is FALSE (row 8), but the
  12 deletions implement `.apm` support with `.agents` preserved in-loop — a feature
  extension, not rot. RED wins "the doc claim is false"; BLUE wins "it's not decay."
- **RED#4 (agent.rs no longer additive):** TRUE and material (row 9). Uphold.
- **RED#5 (fork-health BROKEN):** TRUE and live (row 15). Uphold, highest-confidence.
- **RED#6/#7 (distro platform / executable ledger):** the budget ratchets exist but
  govern fork-*internal* quality, not upstream patches. BLUE's category-error rebuttal
  is correct; these do not violate the divergence model. Downgrade to LOW/noise.

## GREEN's dangerous-drift shortlist (what actually needs action)

1. **BROKEN — fork-health check 5** (row 15): a live invariant failure. Fix now.
2. **DRIFT-BAD — governance gap** (rows 8,9,10,17): agent.rs + swarm/comm + skill.rs
   are real upstream edits with no ledger/seam-audit rows and stale/false doc claims.
3. **DRIFT-BAD — the model's numbers are decalibrated** (rows 1–3): make the baseline
   regenerable so it cannot silently rot again.

Everything else (rows 11,16 and RED#6/#7) is tolerable drift or mislabeled, not a
hardening emergency. The architecture (rows 6,7,12,13,18) HELD.
