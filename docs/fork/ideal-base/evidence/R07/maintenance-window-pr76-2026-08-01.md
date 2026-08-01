# R07 maintenance window — 2026-08-01 (PR #76 / W4 closeout + governance-test decoupling)

PR #76 changed two of the 29 protected governance paths,
`tests/test_ideal_base_railway.py` and `tests/test_governance_compare.py`, so
`Governance Root` failed by design and the section 4 maintenance procedure was
used. Transcript at
[`transcripts/maintenance-window-pr76.txt`](transcripts/maintenance-window-pr76.txt);
window script at [`transcripts/window-pr76.py`](transcripts/window-pr76.py).

## Why this PR needed the window

PR #76 bundled the W4 wave closeout with three governance-test fixes so a single
window covers all of it rather than paying for two:

1. **W4 closeout (non-protected):** marks F24/F25/F27/W4 accepted, adds the F27
   independent-review evidence, and injects six W6 fix nodes (F30-FIX-1..4,
   R05-FIX-1, F26-FIX-1) into WORK_GRAPH.json/STATE.json so the F27 GAP-FOUND
   items are tracked work.
2. **Decoupled the frozen R07 artifact test (protected):**
   `test_state_proposed_json_validates_as_schema_v2` asserted the frozen R07
   hand-off snapshot's node set EQUALS the live graph, so every legitimate graph
   growth (like the W6 injection) broke it and demanded a retro-edit of a
   historical artifact. It now validates the snapshot for self-consistency
   against its own node set (still a valid schema-v2 subset of live).
3. **Remote-name discovery (protected + non-protected):** both governance test
   files hardcoded `refs/remotes/origin/main` / `--fork-remote origin`, failing
   on any checkout whose canonical remote is `github`. They now discover the
   remote, matching the earlier railway fix.
4. **Commit-time graph gate (non-protected):** a pre-commit `railway check`,
   gated on staged WORK_GRAPH.json/STATE.json/validator changes, so a broken
   graph/state edit fails locally in ~8s instead of after a CI round-trip. The
   `--published-ref` ancestry checks stay in CI (they need the remote rail).

Both routes are blocked without a window: with the test edits, `Governance
Root` fails (protected-path change); without them, the over-coupled test rejects
every future graph growth.

## Transaction record

- PR: <https://github.com/jerudnik/jcode/pull/76>
- Reviewed head: `2799e3d3363d32c4341776a1ce37c310d6ad7698`
- Expected base: `395e5e9d11b7b77c77c3f527c74cc5545ef84fe9`
- Required checks on the reviewed head: `Fork CI Gate`, `Security Gate`, and `Nix Gate` **SUCCESS**; `Governance Root` **FAILURE** for the expected protected-path reason. (A `frame_flicker` TUI timing flake in Linux Tests — unrelated to this docs/test-only PR — cleared on a full workflow rerun without a new commit, so the reviewed head stayed stable.)
- Governance Root named two paths, both `tests/`, mechanically verified as the complete protected subset of the PR's 7 changed files.
- Pre-change ruleset 18509013 canonical SHA-256: `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b`, identical to the PR #49/#55/#59/#64/#68 steady-state hash.
- Prospective dropped-body SHA-256: `7e6ba479dde06dace3f169dac9980528f42d90c9b9ed1112cec723a2fe8d35a0`.
- Window opened `08:59:16Z`; only `Governance Root` removed, read back exact.
- Merge conditioned on the exact reviewed head; produced `8260df14ec702e05688f5c328664d3c72cdb6a50`.
- Merge parents exactly `[395e5e9d11b7b77c77c3f527c74cc5545ef84fe9, 2799e3d3363d32c4341776a1ce37c310d6ad7698]`.
- Window closed `08:59:20Z`; literal captured ruleset body restored, canonical hash matched pre-change exactly.
- First-parent history contained exactly one merge in the window, the PR #76 merge.
- Transcript SHA-256: `a5ca1744cf63e426cd9e0736d6055d6c4eaede23bbc00a8f21e7015eb9267f7b`.

## Step 8 live verification

`fork-health.sh --live` at the merge commit `8260df14ec702e05688f5c328664d3c72cdb6a50`: **all invariants hold**, governance snapshot matches the manifest, 29 protected paths, 766 commits over fork-point. `railway check` passes (58 nodes, 66 state records). The protected-path count is unchanged at 29.
