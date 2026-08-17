---
status: open
priority: medium
owner: maintainers
opened: 2026-08-17
related:
  - scripts/governance_compare.py
  - scripts/generate_governance_fixture.py
  - .github/workflows/governance-root.yml
---

# Protected governance comments reference retired documentation

Two comments in the protected governance implementation still reference records removed from `docs/fork/` by the documentation reorganization:

- `scripts/governance_compare.py` names the retired R07 protected-path proposal.
- `scripts/generate_governance_fixture.py` explains the retired checked-in fixture location.

The references are comments only. Current fixture generation already writes to `target/fork-health/`, and governance behavior is unchanged.

## Required maintenance

Update both comments during the next authorized transaction-bound governance maintenance window. An ordinary pull request cannot change either file because `Governance Root` intentionally fails every protected-path diff. Do not weaken or bypass that guard for comment cleanup alone.
