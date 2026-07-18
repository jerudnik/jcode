# Seam ledger template

> **Historical recovery template.** Do not create new recovery seam work from
> this template unless recovery is explicitly reopened. Use
> [`../ideal-base/EXECUTION_PROTOCOL.md`](../ideal-base/EXECUTION_PROTOCOL.md) for
> current node artifacts and review rules.

Create one directory per seam:

```text
seams/<ID>-<slug>/
  opus-review.md
  grok-review.md
  ledger.md
```

Opus and Grok file independent reviews before either sees the other's conclusions. Terra then facilitates evidence-based disagreement and writes the authoritative `ledger.md`. Preserve both independent reviews.

## Independent review template

```markdown
# <ID> <responsibility>: independent review

Reviewer: <model and route>
Baseline: fork `<sha>`; upstream `<sha>`; merge base `<sha>`
Scope checked: <paths, symbols, commits, tests>
Not checked: <explicit gaps>
Research budget: <time or checkpoint limit>
Confidence: <high|medium|low>

## At-a-glance conclusion

| Fork behavior | Upstream behavior | Best disposition | Main risk |
|---|---|---|---|
| <brief> | <brief> | <one disposition> | <brief> |

## Evidence

| Claim | Evidence | Reproduction |
|---|---|---|
| <claim> | <commit/path:symbol/test/incident> | <command or method> |

## Recommendation

<Smallest coherent recommendation, why it is better, and what would falsify it.>

## Strongest case against this recommendation

<Steelman the best competing disposition and name the evidence that would make it preferable.>

## Open questions

- <question and the cheapest decisive check>
```

## Lightweight ledger template

Use this only after the mechanical pre-screen shows low operational risk, little contested divergence, no protected invariant, and no pilot dependency. Fable or the coordinator may escalate it to full review.

```markdown
# <ID> <responsibility>: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `<sha>`; upstream `<sha>`; merge base `<sha>` |
| Review mode | `light` |
| Recommended disposition | <one value> |
| Confidence | <high|medium|low> |

| Finding | Evidence | Consequence |
|---|---|---|
| <brief> | <commit/path:test/command> | <brief> |

- Acceptance or retirement condition: <brief>
- Escalate to full review if: <explicit trigger>
- Coordinator approval: <pending/pass/fail>
- Fable review: <pending/pass/fail>
```

## Authoritative ledger template

```markdown
# <ID> <responsibility>: authoritative ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `<sha>`; upstream `<sha>`; merge base `<sha>` |
| Review mode | `full` |
| Research budget | `<time or checkpoint limit>` |
| Authority today | `<fork|upstream|split|unclear>` |
| Recommended disposition | `<adopt-upstream|retain-fork|compose|upstream-patch|delete|defer>` |
| Confidence | `<high|medium|low>` |
| Last updated | `<UTC timestamp>` |

## Scope and invariants

- Owns: <behaviors, not merely files>
- Excludes: <adjacent responsibilities>
- Must preserve: <observable invariants>

## Divergence at a glance

| Concern | Fork | Upstream | Consequence |
|---|---|---|---|
| Behavior | <brief> | <brief> | <brief> |
| Architecture | <brief> | <brief> | <brief> |
| Tests | <brief> | <brief> | <brief> |
| Operations | <brief> | <brief> | <brief> |

## Evidence ledger

| Finding | Evidence | Confidence | Decides |
|---|---|---|---|
| <brief> | <commit/path:symbol/test/command> | <H/M/L> | <question> |

## Adjudication

| Disagreement | Opus position | Grok position | Terra resolution | Deciding evidence |
|---|---|---|---|---|
| <brief> | <brief> | <brief> | <brief> | <citation> |

Terra reproduction: <one decisive command personally rerun, its result, and what it decided>

## Recommendation

- Disposition: <one value>
- Why: <quality, maintenance, and integration rationale>
- Cross-seam dependencies: <IDs>
- Upstream opportunity: <none or a bounded patch>
- Quality-of-life ideas: <record only; implement in a separate lane>

## Bounded implementation slices

| Slice | Class | Change | Acceptance | Rollback or stop condition |
|---|---|---|---|---|
| 1 | `<sync|fix|refactor|qol|docs>` | <brief> | <tests/observable result> | <explicit trigger> |

## Validation and sign-off

- Commands: <targeted through broad>
- Failure modes checked: <list>
- Remaining risks: <list>
- Opus review: <pass/fail plus evidence>
- Grok review: <pass/fail plus evidence>
- Terra adjudication: <pass/fail>
- Sol sign-off: <pending/pass/fail>
- Fable sign-off: <pending/pass/fail>
```

## Evidence standard

- Prefer symbol-level comparisons and patch IDs over file-level similarity.
- Every patch-equivalence claim records the exact command, refs, options, and assumptions used.
- Distinguish author time, committer time, ancestry, and branch/ref observations.
- Treat `b3ed82a6b` as an ancestry gap. Search for absorbed behavior rather than assuming a commit is absent because it is unreachable.
- Record negative findings and unexamined areas.
- If evidence exists only in an external or untracked note, summarize the decisive facts in the ledger and record the absolute path plus content hash.
- Low confidence cannot pass a gate. Commission a targeted check or mark the seam blocked.
- When a research budget expires, Terra reports the remaining decisive questions. Sol narrows, escalates, or blocks the seam instead of extending it silently.
- Terra is the sole committer for a full seam branch. The coordinator integrates the completed ledger commit into the recovery branch.
