# R07 maintenance window — 2026-08-01 (PR #68 / reload in-place detection)

PR #68 changed two of the 29 protected governance paths,
`.github/workflows/fork-ci.yml` and `scripts/check_critical_path_budget.py`,
so `Governance Root` failed by design and the transaction-bound maintenance
procedure in [`design.md`](design.md) section 4 was used. The exact executable
transcript is preserved at
[`transcripts/maintenance-window-pr68.txt`](transcripts/maintenance-window-pr68.txt);
the window script variant at
[`transcripts/window-pr68.py`](transcripts/window-pr68.py).

## Why this PR needed the window

PR #68 fixes reload detection: the canonical publish flow overwrites
`~/.jcode/current/jcode` in place, so the running exe and the reload candidate
resolve to the same canonical file and the mtime-vs-mtime comparison could never
report a genuinely newer published build. The fix compares same-path candidates
against the process start time (macOS libproc, Linux `/proc/self`).

That new probe code lived in `server/util.rs`, which was already over the
code-size budget, so it was extracted into a new file
`server/util/binary_freshness.rs`. The extraction adds exactly one production
file to the `lifecycle` critical-path scope (`crates/jcode-app-core/src/server/`),
raising its expected file count 63 → 64. The critical-path *gate* itself passes
(no new panics, swallowed errors, or oversized files — the probe was written to
avoid the `.ok()` ratchet and the `manual_ok_err` clippy lint alike), but two
protected artifacts must record the honest structural change:

- `scripts/check_critical_path_budget.py`: `EXPECTED_FILE_COUNTS["lifecycle"]`
  63 → 64, which changes the pinned scope digest to
  `6e9367a924cc4199b0725b6181e78e9a2b9a5bad65eb3be4a2d54fc09d67177d`.
- `.github/workflows/fork-ci.yml`: the `--expect-digest` pin updated to match.

Both routes are blocked without a window: with the edits, `Governance Root`
fails (protected-path change); without them, the pinned-count unit test in
`test_critical_path_budget.py` fails because the scope legitimately grew by one
file. No debt ceiling was raised; only the file-count bookkeeping and its digest.

## Transaction record

- PR: <https://github.com/jerudnik/jcode/pull/68>
- Reviewed head: `ccaa8d0c7e85ca286a6a9d0ddac5840f4e3e09f9`
- Expected base: `35632291a27557af8c6ccefbaba78152d6e0d994`
- Required checks on the reviewed head: `Fork CI Gate`, `Security Gate`, and `Nix Gate` **SUCCESS**; `Governance Root` **FAILURE** for the expected protected-path reason.
- Governance Root named two paths, `.github/workflows/fork-ci.yml` and `scripts/check_critical_path_budget.py`, mechanically verified as the complete protected subset of the PR's 4 changed files.
- Pre-change ruleset 18509013 canonical SHA-256: `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b`, identical to the PR #49/#55/#59/#64 steady-state hash.
- Prospective dropped-body SHA-256: `7e6ba479dde06dace3f169dac9980528f42d90c9b9ed1112cec723a2fe8d35a0`, identical to prior single-context drops.
- Window opened `06:43:41Z`; only `Governance Root` was removed and the write was read back exactly.
- Merge was conditioned on the exact reviewed head and produced `cdf6c869007a9b2e5bfc84c872f8c3db5de6e53d`.
- Merge parents are exactly `[35632291a27557af8c6ccefbaba78152d6e0d994, ccaa8d0c7e85ca286a6a9d0ddac5840f4e3e09f9]`.
- Window closed `06:43:46Z`; the literal captured ruleset body was restored and its canonical hash matched the pre-change hash exactly.
- First-parent history contained exactly one merge in the window, the PR #68 merge.
- Transcript SHA-256: `79f484c26654961e65d88933e65b3f0111a08f151f3c3a666edc89fc668cdfc8`.

## Step 8 live verification

`fork-health.sh --live` was run at the merge commit
`cdf6c869007a9b2e5bfc84c872f8c3db5de6e53d`: **all invariants hold**, governance
snapshot matches the manifest, 29 protected paths, 756 commits over fork-point.
The protected-path count is unchanged at 29: PR #68 edits the bodies of two
already-protected files and does not alter the protected set.
