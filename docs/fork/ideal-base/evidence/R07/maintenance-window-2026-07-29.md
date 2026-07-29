# R07 maintenance window — 2026-07-29 (PR #41 merge)

The barrier-6 evidence PR touched `tests/test_ideal_base_railway.py`
(protected), so its merge used the transaction-bound maintenance procedure
(design.md §4 "Later governance maintenance"). Executed 2026-07-29 with the
admin token read inline via rbw (refreshed after expiry mid-procedure; the
first window attempt failed closed with a 401 before any write).

- PR: https://github.com/jerudnik/jcode/pull/41
- head_sha: `a08a829d011c97dd6c38ac9a1d411d1e886e0058`
- expected_base_sha: `a545ecee42fef172c8c6ff730dc6cb5608ebe652`
- Step 1: Governance Root red on head, naming exactly
  `tests/test_ideal_base_railway.py` (captured from check-run log); Fork CI
  Gate, Security Gate, Nix Gate all green.
- Step 2: ruleset 18509013 body captured; canonical SHA-256
  `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b`.
- Step 3: window opened 04:53:22Z (Governance Root dropped; read-back exact).
- Step 4: merge conditioned on exact head SHA; API merged.
- Step 5: main == merge SHA `493ad30c7cbc8425e1feb466db75af4f436642c8`;
  parents exactly `[a545ecee4, a08a829d0]`.
- Step 6: exact step-2 body restored 04:54:29Z; read-back parsed-equal AND
  canonical-hash identical to step 2.
- Step 7: post-restore main == merge SHA;
  `git rev-list --first-parent --merges a545ecee4..493ad30c7` yields exactly
  one entry, the merge SHA. No other merge or commit landed in the
  67-second window.
