# Normal quality debt register

> **Frozen normalization policy record.** The active quality and security gates
> are materialized in
> [`../ideal-base/WORK_GRAPH.json`](../ideal-base/WORK_GRAPH.json). Do not refresh
> the dated measurements below in place.

> **Policy current, measurements historical.** The no-growth and ownership rules
> remain active. Numeric counts below were measured at the fixed N2 product head
> on 2026-07-16 and must be rerun before being cited as current-tree counts.

Measured: 2026-07-16

Product head: `9fada332799586293ee5d41c6a653ed05efc3821`

This is the normal, non-recovery successor to the R09 ledger. It records debt
without changing the trusted ratchet baselines. The four gate scripts were run
without `--update`; all returned the expected red exit `1`.

## Frozen baseline integrity

| Baseline | SHA-256 |
|---|---|
| `scripts/panic_budget.json` | `aaa2b72dff641c482248676b4b1309cc98fd21934370eec74fb62ee9579cece8` |
| `scripts/swallowed_error_budget.json` | `0b70750a82c17771726cacbe6deeefe807b09058d2e0078d1dfc2c31e8b53dc8` |
| `scripts/code_size_budget.json` | `c7e46062390fa73d3ebfd99217bda289290ab146a104a2d8d501a72bc0c6cd19` |
| `scripts/test_size_budget.json` | `5402e55b096d1b6bb71ea0bc38c39fa3eda98c22290e7d863418d52508997ee6` |

No baseline file changed during normalization. The accepted Phase 6 starting
point remains panic `31 -> 48`, swallowed-error `2987 -> 3074`, production-size
red, and test-size red.

## Exact N2 measurement

| Metric | Baseline | Current | Affected state | Gate | Evidence |
|---|---:|---:|---|---:|---|
| Panic-prone production usage | 31 | 48 | 11 affected Rust paths plus the total-count finding | 1, expected red | `evidence/2026-07-16-n2-readiness/r09-panic.txt` |
| Swallowed-error-like production usage | 2,987 | 3,074 | 54 affected Rust paths; category totals: `.ok()` 1,140, `let _` 1,120, `unwrap_or_default` 814 | 1, expected red | `evidence/2026-07-16-n2-readiness/r09-swallowed-error.txt` |
| Production Rust files over 1,200 LOC | 96 tracked at baseline | 99 current, 200,036 total LOC | 63 ratchet regressions; exact current path/LOC map is attached | 1, expected red | `r09-production-size.txt`, `r09-current-production-size.json` |
| Test Rust files over 1,200 LOC | 32 tracked at baseline | 40 current, 79,843 total LOC | 32 ratchet regressions; exact current path/LOC map is attached | 1, expected red | `r09-test-size.txt`, `r09-current-test-size.json` |

The attached raw outputs are authoritative for every affected path and every
per-file baseline delta. The JSON maps enumerate every currently oversized file,
not only regressions.

## Ownership and policy

| Debt | Owner | Required behavior | Review trigger |
|---|---|---|---|
| Panic-prone usage | Maintainers of the owning crate/module; quality-gate maintainer owns classifier semantics | A touched path must not add a new classified occurrence. Prefer typed propagation or explicit failure handling. | Any changed path appears as a new/grown panic finding. |
| Swallowed-error-like usage | Maintainers of the owning crate/module; quality-gate maintainer owns classifier semantics | A touched path must not add `.ok()`, `let _`, or `unwrap_or_default` without an explicit, reviewed reason. New normalization code must keep the accepted total at or below 3,074. | Any total or per-path growth, or any proposed classifier/baseline change. |
| Production size | Owning crate/module maintainers | Existing oversized files stay flat or shrink. New files must remain at or below 1,200 LOC. Splits must create a named authority rather than move lines mechanically. | The next semantic change to a listed file, or a new oversized file. |
| Test size | Owning crate/module maintainers | Add focused fixture modules rather than extending a listed monolith. A split must preserve fixture discoverability and exact behavior. | The next fixture addition to a listed file, or a new oversized test file. |

For cross-cutting ownership disputes, the normalization/quality maintainer is the
triage owner and assigns the change to the crate whose public authority is being
modified. Debt does not become recovery work again merely because it originated
before normalization.

## Priorities

1. **No regression is mandatory.** The frozen scripts continue to reject growth.
2. **Correctness-triggered cleanup beats shape-only cleanup.** Refactor when an
   adjacent feature or fix supplies a safe boundary and focused fixtures.
3. **New normalization debt is repaired immediately.** During N2 two new `.ok()`
   occurrences temporarily raised swallowed debt to 3,076; commit `98f2697ea`
   removed both and restored 3,074 without updating the baseline.
4. **R02 large files are owned, not hidden.** `provider/mod.rs` (2,797 LOC) and
   `sidecar.rs` (2,235 LOC) retain no-growth obligations. Their broad W7 split
   proposal is closed in [`R03A_R02_CLOSURE.md`](R03A_R02_CLOSURE.md); bounded
   extractions may re-open only under the triggers recorded there.
5. **W7d size is visible.** `jcode-plan/src/lib.rs` is a new 1,368 LOC production
   violation after the bounded-provenance implementation. Its next semantic
   change must extract a named plan-progress authority or otherwise return it
   below the threshold without widening the persistence contract.

## Baseline changes

A baseline update is not a cleanup mechanism. It requires a separate proposal
that includes:

- the exact before/after counts and path map;
- why the classifier or threshold is wrong rather than the source;
- independent review of parser semantics;
- explicit approval; and
- evidence that the update does not erase a regression.

No such update was used or is proposed by N2.
