# R07 maintenance window — 2026-07-29 (PR #49 / F23)

F23 changed seven protected governance paths, so PR #49 used the transaction-bound maintenance procedure in `design.md` section 4. The exact executable transcript, including both step-8 commands and outputs, is preserved at [`transcripts/maintenance-window-pr49.txt`](transcripts/maintenance-window-pr49.txt).

## Transaction record

- PR: <https://github.com/jerudnik/jcode/pull/49>
- Reviewed head: `41b97a1e2eb0f5fbcc3d3ef2a8176079dd6bcf58`
- Expected base: `f3412900f174b8dfb265d868424a04a5edee174e`
- Required checks on the reviewed head: `Fork CI Gate`, `Security Gate`, and `Nix Gate` **SUCCESS**; `Governance Root` **FAILURE** for the expected protected-path reason.
- Governance Root named seven paths, all mechanically verified as a subset of the PR's 12 changed files.
- Pre-change ruleset 18509013 canonical SHA-256: `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b`.
- Window opened `20:30:12Z`; only `Governance Root` was removed and the write was read back exactly.
- Merge was conditioned on the exact reviewed head and produced `2be9f0b229368d1f19e09c10ada17cf7ba3eb5e6`.
- Merge parents are exactly `[f3412900f174b8dfb265d868424a04a5edee174e, 41b97a1e2eb0f5fbcc3d3ef2a8176079dd6bcf58]`.
- Window closed `20:30:18Z`; the literal captured ruleset body was restored and its canonical hash matched the pre-change hash exactly.
- First-parent history contained exactly one merge in the six-second window, the PR #49 merge.

## Step 8 live verification

`fork-health.sh --live` was run with `gh` available from a Nix shell at both boundary commits after restoration:

- Base `f3412900f`: **all invariants hold**, governance snapshot matches the manifest, 27 protected paths.
- Merge `2be9f0b22`: **all invariants hold**, governance snapshot matches the manifest, 29 protected paths.

The 27→29 change is intentional: F23 adds `scripts/check_critical_path_budget.py` and `scripts/test_critical_path_budget.py` to the protected set so the gate cannot attest to its own weakening.
