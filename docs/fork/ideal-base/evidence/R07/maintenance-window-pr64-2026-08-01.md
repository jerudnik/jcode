# R07 maintenance window — 2026-08-01 (PR #64 / railway published-ref discovery)

PR #64 changed `scripts/ideal_base_railway.py`, one of the 29 protected governance paths, so `Governance Root` failed by design and the transaction-bound maintenance procedure in [`design.md`](design.md) section 4 was used. The exact executable transcript is preserved at [`transcripts/maintenance-window-pr64.txt`](transcripts/maintenance-window-pr64.txt); the script that produced it (the PR #59 `window.py` with only PR number, reviewed head, and expected governance path changed) is preserved at [`transcripts/window-pr64.py`](transcripts/window-pr64.py).

## Why this PR needed the window

The 2026-08-01 repository cleanup collapsed the canonical checkout to a single remote named `github` (the duplicate `origin` remote was removed). The railway's hardcoded default published ref `refs/remotes/origin/main` then failed to resolve, so every bare `ideal_base_railway.py check`/`status` invocation on the canonical checkout aborted with `published ref does not resolve`. The fix makes the *default* try known remote names in order (`origin` first, preserving CI, then `github`); an explicit `--published-ref` is never rewritten, and CI passes the flag explicitly, so enforcement semantics are unchanged.

Both available routes are blocked without a window: with the edit, `Governance Root` fails (protected-path change); without it, the local railway tooling stays broken on the canonical checkout, which the ideal-base program depends on for its own state validation.

## Transaction record

- PR: <https://github.com/jerudnik/jcode/pull/64>
- Reviewed head: `97aa4963cdbea479c71112008e94eec89c9ef8cd`
- Expected base: `0640a0fa141622780b795d14f7905119d07f086c`
- Required checks on the reviewed head: `Fork CI Gate`, `Security Gate`, and `Nix Gate` **SUCCESS**; `Governance Root` **FAILURE** for the expected protected-path reason.
- Governance Root named one path, `scripts/ideal_base_railway.py`, mechanically verified as the complete protected subset of the PR's 1 changed file.
- Pre-change ruleset 18509013 canonical SHA-256: `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b`, identical to the PR #49/#55/#59 steady-state hash.
- Prospective dropped-body SHA-256: `7e6ba479dde06dace3f169dac9980528f42d90c9b9ed1112cec723a2fe8d35a0`, identical to the value recorded for the same single-context drop in the PR #55 and PR #59 windows.
- A first `--commit` attempt at `02:46:16Z` opened and safely auto-closed the window (`02:46:20Z`, restore hash exact) without merging: GitHub refused the merge over an unresolved review conversation. Governance was verified restored before the conversation was resolved and the window re-run.
- Window (successful run) opened `02:46:55Z`; only `Governance Root` was removed and the write was read back exactly.
- Merge was conditioned on the exact reviewed head and produced `b28eec443391ce1511e8d02729b7284bbf881998`.
- Merge parents are exactly `[0640a0fa141622780b795d14f7905119d07f086c, 97aa4963cdbea479c71112008e94eec89c9ef8cd]`.
- Window closed `02:47:00Z`; the literal captured ruleset body was restored and its canonical hash matched the pre-change hash exactly.
- First-parent history contained exactly one merge in the window, the PR #64 merge.
- Transcript SHA-256: `45047c2ed3bb895d241174036062dadafa5b1dedf9f2940ea98a51c0e7c6882b`.

## Step 8 live verification

`fork-health.sh --live` was run at the merge commit `b28eec443391ce1511e8d02729b7284bbf881998`: **all invariants hold**, governance snapshot matches the manifest, 29 protected paths, 717 commits over fork-point. `ideal_base_railway.py check` at the same commit reports the graph, state, links, evidence, and protected hash intact — the bare invocation this PR exists to repair.

The protected-path count is unchanged at 29: PR #64 edits the body of an already-protected file and does not alter the protected set.
