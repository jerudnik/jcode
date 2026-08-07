# S03 final ideal-base disposition candidate

## Candidate verdict

**Unqualified ideal-base signoff for the advertised surfaces, pending the
required merge-only publication and post-merge railway checkpoint.**

This is the strongest label permitted by `ACCEPTANCE_STANDARD.md`: all mandatory
deterministic work is complete, G01-G05 are accepted, the final deterministic
matrix passed twice at one fixed runtime commit, and an independent Opus-class
review found zero blockers at the exact S01 evidence commit.

The word "advertised" is essential. This label does not expand platform,
packaging, provider, browser, or distribution claims beyond the accepted gate
records below.

## Exact identities

- Final deterministic runtime subject: `ad7a7d585f77d48dedf47f9e44f6ff838f4405f1`.
- S01 exact evidence subject independently reviewed by S02:
  `8b5263cfaf1d04897ba15d8138341bbbe6f5a330`.
- S02 approval checkpoint: `8eba1833c`.
- Frozen normalizer/prediction commit: `a30670c1f`.
- Pre-publication authoritative main after fetch: `6030441ab`, merge PR #135.
- Reviewed governance fix included in the topic: `1e356391e`.

The exact S03 reviewed commit and final published merge SHA are intentionally not
predeclared. They are populated only after independent S03 review and merge,
respectively.

## Deterministic signoff result

Canonical S01 rounds A and B are consecutive G/H executions at the same runtime
commit. Each contains:

- 18 matrix steps;
- 0 failed steps;
- 577 transcript lines;
- zero owned fixture residue;
- byte-identical F14 restoration.

Both normalize under the frozen contract to:

```
4db50a069513cc2d28c78320713101264f1e635a409b115576a61ea3299f1c52
```

S02 independently recomputed both hashes from `git archive 8b5263cfa`, reran
normalizer controls D1-D4, verified all cited manifests and tracked paths, and
approved with zero blockers. The earlier blocked review remains preserved as a
failed control rather than being rewritten.

## Graph disposition

At this candidate checkpoint:

- 69 nodes are already accepted;
- S01 and S01-FIX-1 are implemented and independently covered by S02;
- S02 is implemented with an APPROVED exact-commit report;
- S03 is implemented by this synthesis and awaits independent review/publication;
- W5 remains in progress until S03 publication;
- F26-FIX-1 remains honestly superseded because measurement falsified its premise.

After publication, the coordinator checkpoint must leave 74 accepted nodes and
1 superseded node. No pending, blocked, in-progress, implemented, or verifying
node may remain.

## External-gate and support disposition

All advertised external gates are accepted:

- **G01:** aarch64-linux was not built or smoked. Documentation explicitly marks
  it best-effort; the label does not imply supported parity.
- **G02:** authorized provider-doctor coverage ran. Full spending tiers passed on
  Claude and the Bifrost bridge. Gemini and Copilot missing-login states retain
  their named `jcode login --provider` next action. The Perplexity route defect
  was owned and repaired by accepted G02-FIX-1.
- **G03:** the packaged browser control surface passed pairing, history,
  send/cancel, reconnect/resync, and stale-ack checks. It is a browser control
  surface, not an installable or secure remote PWA.
- **G04:** Windows and FreeBSD are not supported distribution surfaces, and no
  scheduled installer/release smoke is promised for them.
- **G05:** disposable credential-free Nix/Cachix acquisition and launch succeeded
  for the pinned x86_64-linux artifact. Execution used qemu binfmt emulation,
  not native x86_64 silicon; the public-cache trust requirement is documented.

Distribution remains Nix-only. GitHub release artifacts, native installers, and
the retired iOS product are outside the supported surface.

## Publication and railway checkpoint protocol

Publication is two-phase and must not be collapsed:

1. Independently review the exact commit containing this S03 candidate, the S02
   report, and the current implemented STATE records.
2. Push the topic branch only with explicit authorization.
3. Open a final pull request against fetched `github/main`; wait for every
   required check on the final head. Skipped or absent checks are not green.
4. Merge with a merge commit only. Never squash, rebase, force-push, or reset.
5. Fetch `github/main` and verify:
   - the returned merge SHA is authoritative main;
   - the reviewed S03 candidate, `8b5263cfa`, `8eba1833c`, and `1e356391e` are
     main-ancestral;
   - no required check is red or incomplete.
6. Delete only the merged topic branch; preserve archive/recovery refs.
7. Apply one coordinator-owned local checkpoint commit without pushing, as the
   S03 work-graph contract requires.

The post-merge local checkpoint must:

- mark W5, S01, S01-FIX-1, S02, and S03 `accepted`;
- replace W5's stale five-day-old summary with the actual final dispositions;
- use `8b5263cfa` as the reviewed identity for S01 and S01-FIX-1;
- use the exact independently reviewed S03 candidate commit as the reviewed
  identity for W5, S02, and S03;
- use the final merge SHA as `published_commit` for all five;
- update `last_checkpoint` to S03;
- pass the railway against fetched `github/main`;
- leave the worktree clean after the bounded local checkpoint commit.

## Validation already completed

- Full local `scripts/preflight.sh` passed at runtime subject `ad7a7d585`, with
  rustfmt and Clippy green. The unused-dependency check was unavailable locally
  because `cargo-machete` is absent and remains CI-enforced.
- S01 matrix: 18/18 twice, identical normalized hash, zero residue.
- S01, S01-FIX-1, F03, and F14 manifests: verified.
- `scripts/ideal_base_railway.py check`: 9 roots, 66 child nodes, 75 records,
  protected hash intact.
- Documentation references: 130 active documents, 0 machine-local, 0 stale code
  path at baseline.
- Generated/projected agent instructions: within budget and matching sources.
- Independent S02 exact-commit review: APPROVED, zero blockers.

## Residual limitations and deferred work

These do not weaken the accepted label because they are outside or narrower than
the advertised surface, but they remain explicit:

1. `JCODE_TEST_SESSION` handling is still interleaved through six branches in
   `do_reload`. A dedicated `ReloadEnvironment` seam is deferred until after
   program close; the current fallback is tested and the final matrix covers it.
2. The env-flag truthiness consolidation was a disclosed cross-crate mechanical
   refactor broader than S01-FIX-1's original owned-path list. Future mid-node
   cross-cutting refactors should receive their own graph node.
3. Superseded round logs C-H are locally ignored by the operator's global
   `*.log` rule. Nothing cites them; canonical G/H bytes are tracked as A/B.
   Consider a repository-level evidence-log negation.
4. The full-preflight run has no committed transcript. The exact failure surface
   was independently rerun, but future final gates should retain a bounded log.
5. External provider, cache, and governance state can drift after its recorded
   observation; accepted evidence is a fixed-time proof, not a perpetual claim.
6. The PWA program remains explicitly deferred.

## Edge cases considered

- stale local `main` versus fetched `github/main`;
- main-only content hidden by a merge node;
- accepted-state fields populated before publication;
- skipped/absent required checks mistaken for green;
- squash/rebase destroying reviewed-to-published mapping;
- stale W5 narration contradicting accepted child states;
- normalizer widening after mismatch;
- copied or substituted round transcripts;
- globally ignored evidence cited as tracked;
- unsupported platforms accidentally included in the final label;
- external state drifting after the evidence snapshot.

## Open questions

No technical acceptance question remains. The only pending decision is explicit
authorization for the external GitHub writes: branch push, final PR creation, and
merge. The post-merge STATE checkpoint is local-only by current S03 contract.

## Confidence and what was not rechecked here

Confidence: **high** for the advertised-surface ideal-base disposition, subject
to the publication protocol above.

This synthesis did not rerun the matrix, provider spends, hardware-gated checks,
or package acquisition. It composes accepted node evidence, the two final S01
transcripts, and the independent S02 review. Publication checks and main
ancestry remain unperformed until explicit authorization is granted.
