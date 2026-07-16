# R09 Quality-gate semantics, debt attribution, and ratchet policy: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4` (Phase 1 adjudication baseline; ledger authored at `8848f2d54f67f9a5a1de76bace9666c78036e116`); upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `light overlay` (mandatory quality overlay per `RESPONSIBILITIES.md`) |
| Research budget | 8 decisive checkpoints for the R00/R09/R11 overlay batch; 1 consumed here beyond preserved Phase 0 evidence |
| Recommended disposition | `retain-fork` |
| Confidence | high |

R09 owns trusted classifier behavior, current red-debt visibility, inherited/fork attribution, CI interpretation, and the no-blanket-baseline-update rule. It excludes behavior remediation and synchronization decisions. Phase 0 already repaired and independently approved the classifier semantics (`QUALITY_GATES.md`), which is why this is an overlay rather than a runtime seam. Upstream gate/CI content is not authoritative; the fork's repaired, adversarially tested classifier is the trusted parser.

## Findings

| Finding | Evidence | Consequence |
|---|---|---|
| One shared, independently approved classifier replaced the invalid duplicated one | Commits `fb1168a6a`, `0508e3f7b`, `0674fe53d`, `f9c70d1be` (isolated sources `c3c3dd760`, `0bcb7ca49`, `2456111b5`, `c53022f4d`); reviews `reviews/2026-07-15-gate-parser-{initial,final,rereview}-opus.md`; 17 adversarial tests via `python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'` | Parser semantics are trusted and must not be silently regressed; both gate scripts import the same implementation |
| Parser-semantic correction and stale-ratchet tightening are distinct, both required | `QUALITY_GATES.md`: panic `34 -> 31` at the original baseline is a parser effect (old parser fails with corrected JSON); swallowed `2,988 -> 2,987` restores the value already present at `f67e7b45d...` and is not a parser effect | Baseline changes must be split by cause; a merged "correction" that mixes semantics with rebaselining is a policy violation |
| Trusted greens pass; four gates are red with real debt | Gate truth table (`QUALITY_GATES.md`): warning `0` green, wildcard `16` green, dependency boundaries green through the pinned dev shell; production size 60 violations/+6,604 net LOC, test size 31/+3,679, panic `31 -> 46` (+15), swallowed `2,987 -> 3,077` (+90), all red | Red debt is visible, quantified, and structurally credible; it blocks nothing by being visible and everything by being hidden |
| No command used `--update`; the replay proof anchors the baselines | `QUALITY_GATES.md` reproduction section; archive replay at `f67e7b45d...` with repaired scripts and corrected JSON passes exactly at panic `31` and swallowed `2,987` | The current baselines are provably correct at their origin; any future `--update` would sever that proof |
| Debt is attributed by slice, not owned by R09 | Historical attribution table: curated-sync slice carries 66.7% of panic drift, 73.3% of swallowed drift, 87.0%/88.0% of size-violation LOC growth; fork-only slices carry the remainder | Attribution guides which behavior seam pays which debt; the curated slice is not thereby "upstream's fault," and seam review must separate imported, composed, and fork-specific changes inside it |

## Mandatory overlay obligations on every seam

1. **Trusted parser semantics are preserved.** No seam may modify `scripts/rust_production_filter.py`, `scripts/check_panic_budget.py`, `scripts/check_swallowed_error_budget.py`, or the 17 classifier tests as a side effect of sync or remediation. A genuine classifier defect goes through the same isolated-branch, independent-review path Phase 0 used (`QUALITY_GATES.md` integrated commits table).
2. **No blanket baseline update.** `--update` is forbidden for hiding inherited or new debt (`RESPONSIBILITIES.md` cross-seam invariant 8). A ratchet may move only with (a) a parser-semantic proof at the original baseline, (b) an independently verified stale-baseline correction, or (c) real remediation that lowers the current count, and each in its own commit.
3. **Red debt stays visible and assigned to owning behavior seams.** Production-size, test-size, panic, and swallowed-error reds remain failing until the owning seam remediates them in bounded slices. Attribution follows behavioral ownership, not file location: for example, panic/swallowed drift in provider/config paths belongs to R02, in agent-turn paths to R12, in worker dispatch to R05B. Each behavior seam's ledger must enumerate the red-debt entries it owns before its implementation gate.
4. **CI interpretation.** Both quality workflows run the 17 classifier tests before the two ratchets (`f9c70d1be`). A green pipeline with fewer checks than this is not a trusted green. Gate verdicts from environments other than the pinned `nix develop` shell are advisory for the dependency-boundary and fmt checks.
5. **Pilot gate condition.** Per `RESPONSIBILITIES.md` pilot prerequisite 4: classifier tests and trusted green gates must pass without `--update`, and existing red debt must remain visible and attributed, for the Phase 3 pilot to proceed.

## Reproduction

```bash
python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'   # 17 tests, OK
python3 scripts/check_panic_budget.py            # must fail: 46 vs baseline 31
python3 scripts/check_swallowed_error_budget.py  # must fail: 3,077 vs baseline 2,987
python3 scripts/check_code_size_budget.py        # must fail: 60 violations
python3 scripts/check_test_size_budget.py        # must fail: 31 violations
python3 scripts/check_wildcard_reexport_budget.py  # pass, total 16
bash scripts/check_warning_budget.sh               # pass, 0 vs 0
```

A run in which the four red commands pass without corresponding remediation commits is evidence of a hidden rebaseline and triggers escalation.

## Explicit gaps

- This ledger did not rerun the gate matrix at `8848f2d54`; the counts cited are the preserved `f9c70d1be` truth-gate results, and only documentation commits separate the two heads (`BASELINES.md` truth-gate checkpoint note). Any seam that changes Rust source must rerun the matrix.
- Per-file assignment of the 60 production-size and 31 test-size violations to specific behavior seams (R02/R04/R05B/R12 etc.) has not been enumerated; each behavior seam ledger owes that list before its implementation gate.
- Upstream's own CI/gate evolution since the merge base (132 upstream commits matching R09 keywords per `PRESCREEN.md`) was not semantically reviewed; nothing in it is adopted here.

## Disposition and conditions

- Recommended disposition: `retain-fork`. The repaired, tested, independently reviewed fork gate stack is the only trusted quality instrument in the recovery. Upstream gate content was not evaluated as a replacement and is not authoritative.
- Acceptance or retirement condition: this overlay retires when Phase 6 sign-off confirms all four red ratchets were either remediated by their owning seams or explicitly re-accepted as attributed debt, with zero `--update` events and classifier tests green throughout. Until then it binds every seam once the coordinator approves this ledger.
- Escalate to full review if: any gate script or classifier test changes outside an isolated, independently reviewed branch; any ratchet JSON changes without a split-by-cause proof; a seam proposes `--update` or a "temporary" baseline bump; the archive-replay proof at `f67e7b45d...` stops passing; or gate counts move without a corresponding source change.
- Coordinator approval: pass, 2026-07-15. The 17 classifier tests, warning budget, and wildcard budget passed; panic, swallowed-error, production-size, and test-size gates remained red without `--update`.
- Fable review: pending independent Phase 4 architecture review; this ledger was authored by Fable and cannot self-approve.

## 2026-07-15 G0 current-tree gate amendment

The old truth table and reproduction numbers above remain historical evidence. A fixed-HEAD G0 rerun at `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3` encoded expected exits before invocation and preserved every log under [`../../evidence/2026-07-15-g0-r09/`](../../evidence/2026-07-15-g0-r09/). The command manifest SHA-256 is `267736890c0152b99bb334ddf6197fadf4f7c1feea55e0ec5926d61e97f44a1e`; the directory `SHA256SUMS` manifest hash is `eadb5441bfdf5aef353a2356b2f04454a33912924a07c8eb7e207146ba992614`.

| Gate | Expected exit | Actual exit | Current result |
|---|---:|---:|---|
| Classifier | 0 | 0 | 17/17 |
| Panic-prone | 1 | 1 | 46 versus baseline 31 |
| Swallowed-error-like | 1 | 1 | 3,074 versus baseline 2,987 |
| Production size | 1 | 1 | 61 findings |
| Test size | 1 | 1 | 31 findings |
| Wildcard re-export | 0 | 0 | total 16 |
| Warning budget | 0 | 0 | current 0, baseline 0 |
| `bash -n scripts/*.sh` | 0 | 0 | pass |
| `git diff --check` | 0 | 0 | pass |

No command used `--update`. Historical swallowed counts `3,077` in Phase 0 and `3,072` in an intermediate log are not erased; the current claim is `3,074` because it reproduced at the fixed G0 HEAD with unchanged preservation state. The direct Cargo-missing dependency failure, two Python-stub failures, one wrong-architecture Python failure, the passing architecture-compatible pinned run, and the initial nonexistent-script shell invocation are all preserved under [`../../evidence/2026-07-15-final-r09/`](../../evidence/2026-07-15-final-r09/) with manifest SHA-256 `113817813b49815d00a10b716e66ab3ed094b28ff6d02fcc60c6d8584c70940a`.

Trusted greens remain green and known debt remains visibly red. This satisfies the evidence precondition for independent G2 review but is not itself pilot authorization.

## 2026-07-15 W0 Phase 4 and count-history amendment

The stale Fable-pending line is discharged. Corrected cross-seam Fable plan SHA-256 `b0bae9803fa726a489e0560fdc423daefa20bd8478ede0aa2772f7684ea21eb9` retained R09 as a binding `retain-fork` quality/debt overlay; independent fixed-plan Opus review SHA-256 `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92` returned **PASS**.

Production-size count `60` above is historical truth at the Phase 0 snapshot head. The fixed G0 and G4 heads record `61`; their expected/actual exit remained `1/1`. Neither count is rewritten or treated as green, and no command used `--update`. Future slices use the newest fixed-HEAD count while preserving the older measurement as dated evidence.

## 2026-07-16 Phase 6 coordinator-audit amendment

The final coordinator matrix at source head
`51168d16e9c708ae4afff09a6fc6402642d17782` reproduced the trusted exit policy:
classifier 17/17, dependency boundaries, wildcard total 16, warning 0, shell
syntax, and diff check passed. Panic remained expected-red `31 -> 48`,
swallowed-error remained expected-red `2987 -> 3074`, and production-size and
test-size remained expected-red. No baseline update was invoked.

The accepted logs and manifest are under
[`../../evidence/2026-07-16-phase6-final-audit/`](../../evidence/2026-07-16-phase6-final-audit/),
`SHA256SUMS` SHA-256
`9af58f1563f266066edd6da9208983da62eeb0b1997ec78f9c26318221dcd2a3`.
The coordinator explicitly re-accepts the four red ratchets as visible,
attributed recovery debt rather than hidden regressions. R09 remains active only
until the required independent reviews and joint Sol/Fable sign-off are
preserved.

## 2026-07-16 spot-check metadata correction

The candidate package hash above remains preserved as reviewed. The independent
spot checker returned PASS and corrected only the label from 76 entries to 62
real checks encoded across 76 physical TSV lines. Corrected metadata has package
`SHA256SUMS` SHA-256
`ca8ff5b9f3b6c09dc0ff05de9b3c1c426fc2373706eeeca26cad87126f2e14d8`.
No expected exit or debt count changed.

## 2026-07-16 architecture-review debt amendment

Independent Fable architecture review returned PASS with zero IMPORTANT or
CRITICAL findings, report SHA-256
`3fa06d1109c5fc56c9cf1bc73dcea540cff084b5ef4fcc1a0a8dcd48e3910865`.
It confirmed that the expected-red production/test-size and panic debt remains
real and visible. W1's duplicated provider-evidence blocks make the W7 R12
consolidation ripe; `append_progress_provenance` also needs a future bounded-size
fixture. These LOW items remain assigned to their behavior owners, not R09, and
their exact triggers are recorded in `RECOVERY_PLAN.md` section 17. No baseline
or expected exit changed.

## 2026-07-16 final retirement amendment

**Status: retired as a special Phase 6 overlay.** Joint Sol/Fable sign-off at
fixed head `17586246a` returned PASS with zero unresolved IMPORTANT or CRITICAL
findings. Report SHA-256 values are
`228f5937dd7eafa6570ed857b3a8db43a1ed43c0a3c9ad6dcaf6e2d29ef8ebe4` and
`7da9ca6810bde9db1035b68e1d2a46f3c0966c6610db7c19553acc96cacc13d3`.

Trusted green gates remain green. Panic `31 -> 48`, swallowed-error
`2987 -> 3074`, production-size, and test-size debt remains visible as
expected-red with no baseline update or attribution laundering. W7 architecture
debt remains assigned and trigger-bound. R09's ratchet and debt-visibility rules
continue under normal quality governance; only the special recovery overlay is
closed.
