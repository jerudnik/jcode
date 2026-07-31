# R07 maintenance window — 2026-07-31 (PR #59 / remove launch-hotkey feature)

PR #59 changed `scripts/ambient_roots_allowlist.txt`, one of the 29 protected governance paths, so `Governance Root` failed by design and the transaction-bound maintenance procedure in [`design.md`](design.md) section 4 was used. The exact executable transcript is preserved at [`transcripts/maintenance-window-pr59.txt`](transcripts/maintenance-window-pr59.txt).

## Why this PR needed the window

Removing the launch-hotkey feature deleted the `dirs::home_dir()` call at `crates/jcode-base/src/config/config_file.rs:834`. That call was allowlisted for the F29 ambient-roots ratchet, so its allowlist entry became stale.

Both available routes are blocked without a window, so the window is unavoidable rather than one option among several:

- **With** the allowlist edit: `Governance Root` fails (protected-path change).
- **Without** the allowlist edit: `scripts/check_ambient_roots.sh` exits 1 on the stale entry, failing the `quality` job, which the required `Fork CI Gate` requires to succeed.

The diff only ever *removes* a permission. After this PR there are zero `dirs::home_dir()` call sites in `config_file.rs`; the ambient-roots allowlist shrinks from 22 entries to 21. This is a ratchet tightening, not a relaxation.

## Transaction record

- PR: <https://github.com/jerudnik/jcode/pull/59>
- Reviewed head: `ac979d4d42ec812f59a529af6771ff21c60eaf6a`
- Expected base: `bd273d66e3048a57c4237fb4b34bd00a46ca240f`
- Required checks on the reviewed head: `Fork CI Gate`, `Security Gate`, and `Nix Gate` **SUCCESS**; `Governance Root` **FAILURE** for the expected protected-path reason.
- Governance Root named one path, `scripts/ambient_roots_allowlist.txt`, mechanically verified as a subset of the PR's 27 changed files.
- Pre-change ruleset 18509013 canonical SHA-256: `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b`, identical to the PR #49 and PR #55 steady-state hash.
- Prospective dropped-body SHA-256: `7e6ba479dde06dace3f169dac9980528f42d90c9b9ed1112cec723a2fe8d35a0`, identical to the value PR #55 recorded for the same single-context drop.
- Window opened `22:32:43Z`; only `Governance Root` was removed and the write was read back exactly.
- Merge was conditioned on the exact reviewed head and produced `d00ef387dae5bbf80d7a8615e7dc08935c4865ab`.
- Merge parents are exactly `[bd273d66e3048a57c4237fb4b34bd00a46ca240f, ac979d4d42ec812f59a529af6771ff21c60eaf6a]`.
- Window closed `22:32:49Z`; the literal captured ruleset body was restored and its canonical hash matched the pre-change hash exactly.
- First-parent history contained exactly one merge in the window, the PR #59 merge.

## Step 8 live verification

`fork-health.sh --live` was run at both boundary commits.

- Base `bd273d66e` (captured pre-merge, read-only): **all invariants hold**, governance snapshot matches the manifest, 29 protected paths.
- Merge `d00ef387dae5bbf80d7a8615e7dc08935c4865ab`: ``all invariants hold`, 29 protected paths, 702 commits over fork-point`

The protected-path count is unchanged at 29: PR #59 edits the body of an already-protected file and does not alter the protected set.

## Window script provenance

`window.py` was not committed by the PR #49 or PR #55 windows, so it was reconstructed from the PR #55 transcript and is committed here as `transcripts/window.py` to stop the next window from repeating the reconstruction.

The reconstruction was validated against independently recorded values rather than by inspection alone:

- The dry run reproduces **both** hashes recorded in the PR #55 transcript (`43ba61a7…` pre-change and `7e6ba479…` dropped-body).
- Step 7's commit-range query was back-tested against PR #55's actual recorded window (`21:56:12Z`–`21:56:18Z`), returning exactly `c1695a7b442d85e1f315e3c3df8ba2b13583082c`.

Six audit passes found sixteen defects (D1-D16), recorded in full in
[`transcripts/window-audit-pr59.md`](transcripts/window-audit-pr59.md). Almost all
were in code paths a `--dry-run` never executes: with `DRY` set the script returns
at step 2, so steps 3-7 (the entire post-write path, including the restore) are
never reached by the check that most invites treating them as verified.

Four findings are load-bearing enough to name here:

1. **Step 7 was missing entirely** (pass 1). The recorded procedure runs steps 0-8;
   the first reconstruction had 0-6 and 8, omitting the check that exactly one
   merge landed while `main` was unguarded.
2. **The restore was a single non-retrying call, and there were two of them**
   (pass 1). A transient API failure during restore would abort with branch
   protection still dropped. There is now one restore path, retried five times
   with backoff, escalating to an explicit "`main` may still be UNGUARDED" error.
   Verified by fault injection: two simulated 502s, restore succeeded on attempt 3.
3. **Step 7 used a timestamp query instead of the mandated commit-range walk**
   (pass 2, D1). `design.md` section 4 specifies
   `git rev-list --first-parent <base>..<post_restore_main>`; the reconstruction
   used `commits?since=<window_open>`, which counts multi-parent commits *inside*
   the merged branch and depends on wall-clock skew between the API and the
   window. Differential-tested against the last 40 merges on `main`: the two
   methods **disagree on 5**, and at `2be9f0b22` the timestamp query returns 6
   where the mandated walk returns 1. That is a false CONCURRENT MERGE abort,
   raised after the merge has landed and while governance is dropped, in exactly
   the state where a spurious alarm is most damaging. PR #55 happens to be a case
   where both methods return 1, which is why back-testing against it passed.
4. **Two checks would have reported FAILURE on a fully successful window**
   (pass 4, D11). The verifier read local `git` state, but `window.py` merges
   server-side, so local `main` is stale the instant the window closes. It would
   have read the pre-merge allowlist, counted 22 where 21 was expected, and
   declared failure with governance freshly and correctly restored. Both checks
   now read the merge commit through the API.

The remaining twelve are of the same character: fail-open scans, a resolution
path that crashed from the staging directory, an exit code that collapsed
"cannot run" into "failed", and abort messages that named a symptom without
naming the recovery action. **Four of the sixteen are defects introduced by
earlier fixes in this same audit**, which is the strongest argument in the record
for committing this script rather than reconstructing it a fifth time.

Because a dry run cannot reach the post-write path, that path was checked by
other means: `py_compile`, an AST pass asserting every loaded name is bound,
fault injection at the restore and at the governance-path guard, back-testing
step 7 against PR #55's real recorded window, cross-validation of the mutation
against `scripts/governance_compare.py --live` (an independent, 74-test-backed
oracle that names the failure rather than reporting a hash difference), and
rendering every operator-facing abort message as text.
