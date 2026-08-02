# F30-FIX-3: close the workflow lint-list completeness gap

Reviewed commit `d1977771e`, published in merge `b88250783` (PR #90).

## The node named three workflows. Only one is a defect.

`distributionPolicySrc` covers all of `.github/workflows`, but both actionlint
invocations (`nix.yml` and the flake workflow-syntax check) named seven
workflows explicitly. The node reports `ci.yml`, `freebsd-smoke.yml`, and
`governance-root.yml` as missing.

`ci.yml` and `freebsd-smoke.yml` were both added upstream: their adding commits
are ancestors of the `fork-point` tag, and `nix.yml` already documents why they
are excluded (kept byte-close to the fork point; dispatch-only). That rationale
is sound and is preserved rather than overturned to make the count come out
even.

`governance-root.yml` is different. It landed in `07b10b1e2` on 2026-07-28, is
not an ancestor of `fork-point`, and is therefore fork-owned. It falls squarely
inside the stated rule and was simply never added, because **the rule lived in a
comment while enforcement lived in a hand-maintained list**. The work graph was
recorded 2026-07-18, ten days before this workflow existed, so the node could
not have named it as fork-owned.

## The fix

Add it to both actionlint lists and to `checkSrc`, then replace the implicit
rule with `test_every_fork_owned_workflow_is_lint_covered`, which enumerates the
workflows actually present and requires each one outside an explicit exemption
set to appear in both lists. The exemptions are themselves asserted to still
exist, so a dead exemption cannot linger and quietly widen the hole.

That is the durable part: a comment describing a rule became a test enforcing
it, so the next fork-owned workflow cannot be forgotten the same way.

## Verification

Control, observed failing: removing `governance-root.yml` from the `nix.yml`
list alone FAILS with

```text
fork-owned workflow governance-root.yml is not linted by nix.yml
```

actionlint passes on all eight fork-owned workflows. Suite went 11 -> 12 tests;
hermetic derivation green. Re-verified on published `main`: 13/13 OK.
