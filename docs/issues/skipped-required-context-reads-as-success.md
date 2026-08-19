---
title: A skipped required context is reported as success, and this repository cannot observe that itself
status: open
priority: high
owner: unassigned
opened: 2026-08-19
---

# A skipped required context is reported as success, and this repository cannot observe that itself

## Summary

GitHub documents that `success`, `skipped` and `neutral` all satisfy a required
status check, and that a job skipped by a conditional reports Success. So a
pull request that gives a required job a condition which never fires turns the
gate green rather than red.

Three specific instances of that were closed in #187, #188 and #189. What
remains open is stated here, because closing an instance is not closing a class
and the residue is worth naming rather than implying.

## What is closed, so it is not re-investigated

- A rename routed by its destination only, so moving a source file into the
  prose directory skipped every product leg. Fixed in #187 by reading both ends
  of a rename.
- The workflow `if:` contract and the routing classification table were each
  enforced at pull-request time by exactly one unprotected test. Fixed in #188
  by asserting both from the protected guard registry, and in #189 by adding
  those two tests to the protected path set.
- The guard registry counted a mention as wiring, and three guards were
  registered dormant while running on every pull request. Fixed in #188.

## What is still open

### 1. The platform behaviour has never been observed here

That a skipped required context is treated as success rests on GitHub's
published rule, not on a run in this repository. Observing it directly requires
opening a pull request that disables the gate, which is the thing the rule
forbids doing.

The narrower question is also unsettled: GitHub states the rule without
distinguishing rulesets from classic branch protection, and this repository
uses a ruleset. The plants added in #188 make the local half fail, which is
what actually matters for a merge, so this is recorded rather than pursued.

### 2. Failure capability is unestablished for most checks

Over the twelve merged pull requests preceding this note, only `Governance Root`
was ever observed concluding failure. Every other context, including `PR Gate`
and each routed leg, has only ever been observed passing or skipping at a merged
head.

A check never observed failing is a check whose ability to fail is unproven.
That is the same standard this repository applies to its guards, turned on the
evidence base rather than the code, and it has no mechanism behind it yet.

### 3. One classifier route had almost no production record

The classifier has three reachable outcomes. Replaying it over the twelve merged
pull requests preceding this note, two occur and the third does not: the route
where a change is not prose but is judged unable to affect the built artifact.
That route skips the package build, the smoke check, the full-test recipe and
the release-check step.

Corrected the same day, by the fix sequence itself. #189 changed only
`.github/workflows/governance-root.yml` and `scripts/required-checks.json`, both
of which the table treats as inert, so it routed
`docs_only=false, product_impacting=false` and became the first observed
instance. Its check-runs show `Build Nix package` and `Smoke` skipping while
`Nix Validate` and `Fork CI / Rust checks` ran, which is the route behaving as
designed.

So the route is no longer unobserved. What remains is that a single instance is
a thin basis, and that nothing arranges for it to be exercised: the next twenty
pull requests could all be prose or all be product changes and the route would
go untested again without anyone noticing.

### 4. The class, not the instance

Both fixed instances had the same shape: one property, one detector, and the
detector inside the blast radius of the change it judged. Nothing currently
detects a new instance of that shape. A property added tomorrow with a single
unprotected detector would reproduce it, and the audit that found these two was
manual.

## Why this is filed rather than fixed

Items 1 and 3 need a pull request that deliberately reddens a required gate,
which is a governance decision rather than an engineering one. Item 2 needs a
policy on how to establish failure capability without breaking main. Item 4
needs a way to enumerate single-detector properties, which is a design question
and not a small one.

Each is cheap to state and expensive to guess at later, which is why they are
written down instead of carried.
