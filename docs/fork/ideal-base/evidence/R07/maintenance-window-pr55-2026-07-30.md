# R07 maintenance window — 2026-07-30 (PR #55 / nix cache + HM installPackage)

PR #55 changed `.github/workflows/nix.yml`, one of the 29 protected governance paths, so `Governance Root` failed by design and the transaction-bound maintenance procedure in [`design.md`](design.md) section 4 was used. The exact executable transcript is preserved at [`transcripts/maintenance-window-pr55.txt`](transcripts/maintenance-window-pr55.txt).

## Transaction record

- PR: <https://github.com/jerudnik/jcode/pull/55>
- Reviewed head: `a8b9248f0c195fdd89af0da7828bf0ea48f5991c`
- Expected base: `df39b1600bb7b5468a9eace2f262c015f399367b`
- Required checks on the reviewed head: `Fork CI Gate`, `Security Gate`, and `Nix Gate` **SUCCESS**; `Governance Root` **FAILURE** for the expected protected-path reason.
- Governance Root named one path, `.github/workflows/nix.yml`, mechanically verified as a subset of the PR's four changed files.
- Pre-change ruleset 18509013 canonical SHA-256: `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b`, identical to the PR #49 window's steady-state hash.
- Window opened `21:56:12Z`; only `Governance Root` was removed and the write was read back exactly.
- Merge was conditioned on the exact reviewed head and produced `c1695a7b442d85e1f315e3c3df8ba2b13583082c`.
- Merge parents are exactly `[df39b1600bb7b5468a9eace2f262c015f399367b, a8b9248f0c195fdd89af0da7828bf0ea48f5991c]`.
- Window closed `21:56:18Z`; the literal captured ruleset body was restored and its canonical hash matched the pre-change hash exactly.
- First-parent history contained exactly one merge in the six-second window, the PR #55 merge.

## Step 8 live verification

`fork-health.sh --live` was run at both boundary commits after restoration:

- Base `df39b1600`: **all invariants hold**, governance snapshot matches the manifest, 29 protected paths.
- Merge `c1695a7b4`: **all invariants hold**, governance snapshot matches the manifest, 29 protected paths.

The protected-path count is unchanged at 29: PR #55 edits the body of an already-protected workflow and does not alter the protected set.

## Content verification

The merged tree was compared against the reviewed head for every touched file. All four blobs are identical, and the range contains exactly two commits (the change and its merge):

| File | Merged vs reviewed |
| --- | --- |
| `flake.nix` | identical |
| `.github/workflows/nix.yml` | identical |
| `nix/modules/home-manager.nix` | identical |
| `docs/NIX.md` | identical |

## Aborted first attempt (no governance impact)

An earlier execution on the same PR aborted at the step-3 read-back and self-restored without merging. The cause was in the window script, not in governance: the canonical hash was taken over the **whole ruleset API response**, which carries server-managed volatile metadata (`updated_at`, `node_id`, `_links`, `current_user_can_bypass`, `source`, `source_type`). GitHub bumps `updated_at` on every write, so a post-write body could never hash-equal its pre-write capture, and the fail-closed restore check reported a mismatch against a value that was wrong to begin with.

Live governance was unaffected. The ruleset was restored correctly by the abort path, and the semantic body continued to hash to `43ba61a7…`. The window was open for roughly two seconds and no merge occurred inside it.

Two corrections were made before re-running, both visible in the transcript:

1. The canonical hash is now taken over the **semantic body only**: exactly the six fields the `PUT` controls (`name`, `target`, `enforcement`, `bypass_actors`, `conditions`, `rules`). This is what reproduces design.md's target body and the PR #49 recorded hash.
2. The step-2 capture is asserted equal to the known-good steady-state hash **before any write**. A hashing defect now stops the procedure with zero writes rather than opening a window it cannot verify.

The dry run was re-executed first and confirmed step 2 producing `43ba61a7…` prior to the live run.
