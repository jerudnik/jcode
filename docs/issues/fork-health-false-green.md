---
status: open
priority: critical
owner: maintainers
opened: 2026-08-15
related:
  - .github/workflows/fork-health.yml
  - scripts/fork-health.sh
---

# Hosted fork-health governance check is a false green

The scheduled and manually dispatched `fork-health` workflow reports success even when its live governance comparison cannot run. The guard exits 2 because `RULESET_AUDIT_TOKEN` is undefined, but the workflow pipes it through `tee` without `pipefail`, so `tee` masks the failure.

## Required decision and fix

1. Add `set -o pipefail` to the workflow step so a failed guard fails the job.
2. Either define a ruleset-read token that includes `bypass_actors`, or formally make the governance comparison local-only and remove the nonfunctional hosted leg.
3. Decide whether the production contract's reduction from 27 protected paths and four contexts to five paths and two contexts is intentional. Restore coverage or document the accepted reduction.
4. Decide whether the roughly 24-hour detection lag for an unprotected `fork-point` tag is accepted, or add tag-push detection/protection.

Until the first two items are resolved, hosted fork-health results do not prove the governance invariant.
