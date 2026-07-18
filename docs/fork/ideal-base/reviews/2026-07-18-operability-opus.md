# Final adversarial operability review: ideal-base railway

- Reviewer role: independent Opus-class verification / final signoff.
- Date: 2026-07-18.
- Repository: `/Users/jrudnik/labs/jcode`.
- Current HEAD: `923c6353e04266f71dc6cc06fc8516e502a9c07f`
  (`docs(fork): refresh current operating state`).
- Scope of review: the **full uncommitted worktree diff** against HEAD, not a
  committed tree. All ideal-base artifacts (`WORK_GRAPH.json`, `STATE.json`,
  scripts, docs) are currently **untracked/modified** and were reviewed as they
  exist on disk. This review supersedes the earlier PASS whose two MINOR findings
  were reported fixed; both are re-verified below.

## Verdict

**PASS.**

No CRITICAL or IMPORTANT issue found. The control plane is internally
consistent, the validator has real teeth on every safety-relevant invariant, the
native swarm schema/code accepts `deep`/`light`, and a completely fresh session
can execute the program from the repository alone with no prior chat context.

## What was reviewed

Changed/untracked files inspected in full:

- `docs/fork/ideal-base/**` (README, BASELINE, ACCEPTANCE_STANDARD,
  AUDIT_COVERAGE, EXECUTION_PROTOCOL, COORDINATOR_BOOTSTRAP, DECISIONS,
  WORK_GRAPH.json, STATE.json, evidence/README, reviews/README).
- `docs/fork/archive/README.md`.
- Archive/banner markers added to 14 historical docs under
  `docs/fork/normalization/**` and `docs/fork/recovery/**` plus `docs/fork/README.md`.
- `scripts/ideal_base_railway.py` (620 lines).
- `tests/test_ideal_base_railway.py` (112 lines, 7 tests).
- `crates/jcode-app-core/src/tool/communicate.rs` (schema `mode` enum/description).
- `crates/jcode-app-core/src/tool/communicate_tests.rs` (schema assertions).
- Supporting native code confirming the contract is real:
  `crates/jcode-app-core/src/server/comm_graph.rs`,
  `crates/jcode-plan/src/dag/mod.rs`,
  `crates/jcode-swarm-core/src/lib.rs`.

## Validation performed (read-only / non-mutating)

1. `python3 scripts/ideal_base_railway.py check` -> `OK: 6 roots, 38 child nodes,
   44 state records, protected hash intact`.
2. `python3 scripts/ideal_base_railway.py status` and `next --json` ->
   only runnable node is `W0: seed_and_expand`, matching the initial-wave
   projection.
3. `python3 tests/test_ideal_base_railway.py` -> `Ran 7 tests ... OK`.
4. Protected prompt: `shasum -a 256 docs/fork/recovery/ORCHESTRATOR_PROMPT.md`
   == `ca3f19980b1e4fab0a734397d7c6f41ccd5c203a4fa209cfe9eef2f16beed5b6`
   (matches `scripts/ideal_base_railway.py:27` and `BASELINE.md:50`);
   the file is not in the worktree diff.
5. A25 coverage: `audit_coverage` ids are exactly `A01..A25`; the covered F/G
   node set equals the executable node set (32 nodes) with `missing=[] extra=[]`
   (`scripts/ideal_base_railway.py:333-357`).
6. Guard-rail negative tests, all returned exit 1 and wrote nothing:
   - `authorization_blocked` on non-gated `F01` rejected
     (`communicate`... `ideal_base_railway.py:522-526,396-402`).
   - `accepted` without `--commit` rejected (`:533-535`).
   - `accepted` with nonexistent evidence rejected (`:540-542`).
   - Non-RFC3339 and timezone-naive `--updated-at` rejected (`:527-532`).
   - Unknown node id rejected (`:518-519`).
   After all four, `STATE.json` was byte-unchanged and every node still `pending`.
7. Atomic checkpoint + resume happy path (executed on a backed-up copy, then
   restored): `checkpoint W0.1 --state accepted --commit <reachable> --evidence
   <existing>` succeeded, re-validated, and the temp file was cleaned; `next`
   remained correct. State restored to all-`pending` afterward.
8. Ownership "teeth" tests via importing the module:
   - Removing `W2 -> W1` makes `F02`/`F10` (`server/**`) overlap and is
     `REJECTED: unserialized ownership overlap` (`:234-245`).
   - Removing `W4 -> W3` makes cross-parent overlap (`F02`/`F25` on
     `server/lifecycle.rs`) `REJECTED`, proving cross-subtree serialization is
     enforced (`ordered()` at `:212-221`).
9. Native contract cross-check: `WORK_GRAPH.artifact_schema.required` ==
   `{findings, evidence, edge_cases_considered, validation, open_questions,
   confidence, what_i_did_not_check}` == fields of `HandoffArtifact`
   (`crates/jcode-plan/src/dag/mod.rs:259-284`); deep mode requires `findings`
   and `what_i_did_not_check` (`mod.rs:256`), matching
   `EXECUTION_PROTOCOL.md:76-96`.
10. Schema/code for `mode`: enum is `["all","any","deep","light"]` with a
    task_graph-aware description (`communicate.rs:2301-2302`), asserted by
    `communicate_tests.rs:1425-1440`. Server honors it and, crucially, refuses a
    silent deep->light downgrade of a non-empty plan
    (`server/comm_graph.rs:243-283`), and defaults to deep from session effort
    when omitted. `deep`/`light` are exercised end-to-end in
    `server/comm_control_tests/dag_e2e.rs`.

## Requirement-by-requirement findings

- **Exact A01-A25 coverage:** PASS. Ordered ids and F/G-only citations enforced;
  `covered == executable` proven programmatically. `AUDIT_COVERAGE.md` mirrors
  the machine map.
- **Deep-mode seed operability + published schema accepts deep/light:** PASS.
  Schema, deserializer (`communicate.rs:2090` `mode: Option<String>`), server
  resolution, and downgrade guard all present. Bootstrap instructs explicit
  `mode: "deep"` (`COORDINATOR_BOOTSTRAP.md:53-56`, `EXECUTION_PROTOCOL.md:23`).
- **Native complete_node contract:** PASS. `HandoffArtifact` fields and deep-mode
  requirements match the graph's `artifact_schema` and protocol doc exactly.
- **Gate-cap safety:** PASS. `root_nodes<=10` and each `expansions[*]<=10`
  enforced (`:288-297`); actual roots=6, max expansion=8.
- **DAG dependencies:** PASS. Kahn topo-sort with explicit cycle report, self-dep
  and unknown-dep rejection (`:131-162`); real graph is acyclic
  (W0<-W1<-...<-W5 plus intra-wave edges).
- **Exact and glob-subsumption write ownership:** PASS. `ownership_paths_overlap`
  handles exact, glob-vs-literal (`fnmatch`), and glob-vs-glob prefix subsumption
  (`:183-202`); serialization allowed only along dependency closure or ancestor
  root closures. Both negative tests
  (`test_unserialized_exact_path_overlap_is_rejected`,
  `test_unserialized_glob_subsumption_is_rejected`) pass, and I independently
  broke real edges to confirm rejection.
- **Coordinator-only STATE/DECISIONS authority:** PASS.
  `coordinator_owned_paths = [STATE.json, DECISIONS.md]` and no child
  `owned_paths` intersects them (`:320-330`); asserted in
  `test_repository_control_plane_is_valid`. `EXECUTION_PROTOCOL.md:51-54` states
  the coordinator alone writes them and workers only propose.
- **Atomic checkpointing:** PASS. `atomic_write_json` writes a temp sibling,
  `fsync`s file and parent dir, `os.replace`, cleans temp
  (`:495-513`); guarded by a git-dir `flock` around read-modify-write
  (`:543-573`) with a re-validate after write. `test_atomic_json_write_is_complete`
  verifies no residue.
- **Resume semantics:** PASS. `ready_nodes` recomputes runnable work purely from
  `STATE.json` + graph, seeds a pending root only when its deps are
  DEPENDENCY_COMPLETE, dispatches only pending children with complete deps, and
  emits `synthesize` when all children complete (`:422-455`).
  `EXECUTION_PROTOCOL.md:127-137` mandates reclassifying orphaned in-flight nodes
  as `blocked`.
- **Authorization boundaries:** PASS. `authorization_blocked` restricted to
  `class == gated` in both validate and checkpoint paths; the 5 gated nodes each
  name an `authorization` boundary (`WORK_GRAPH.json`), enforced at
  `:316-319`. Docs consistently state blocked != passing
  (`ACCEPTANCE_STANDARD.md:90-101`, `README.md:67-73`, `DECISIONS.md D005`).
- **PM-surface compliance:** PASS. Completed states require a reachable commit
  (`git cat-file -e`) and existing evidence paths (`:384-395`); `DEPENDENCY_COMPLETE`
  = {accepted, authorization_blocked, superseded}. Labels table forbids
  overclaiming (`ACCEPTANCE_STANDARD.md:116-122`).
- **Protected historical integrity:** PASS. All 14 historical-doc edits are
  additive banners only (`1 file changed, 4-5 insertions(+)` each, verified via
  `git diff --stat`); no dated fact rewritten. `ORCHESTRATOR_PROMPT.md` is not in
  the diff and its hash matches. Markdown-link validation covers the control
  plane plus every archive marker and rejects broken/escaping links (`:248-265`).

## Fresh-session executability

**Yes.** A completely fresh coordinator session with no prior chat context can
execute this program. `COORDINATOR_BOOTSTRAP.md` is a self-contained copy-paste
prompt naming every file to read, the exact bootstrap commands, seeding via
`swarm task_graph mode:"deep"`, the checkpoint/commit loop, and the explicit
statement that `STATE.json`, reachable commits, and accepted evidence, not chat
memory, are the restart authority (`COORDINATOR_BOOTSTRAP.md:78-79`,
`EXECUTION_PROTOCOL.md:1-5,127-137`). `BASELINE.md:16` records the seed commit
literally, and the validator/tests run green from a clean checkout.

## Re-verification of the two previously fixed MINOR findings

- The glob-subsumption ownership case is now a first-class test
  (`tests/test_ideal_base_railway.py:65-86`) and passes; the on-disk test file
  contains 7 tests (an earlier stale file cache briefly showed 6, corrected by
  re-reading from disk).
- The `mode` schema now advertises and asserts `deep`/`light` with a
  task_graph-aware description, matched by native server handling.
Both are resolved.

## Confidence

**High** for everything deterministically checkable from the repository:
graph/state integrity, validator correctness and teeth, schema/code alignment,
atomicity, resume logic, ownership serialization, and historical-integrity of the
diff. The Python validator and its tests were executed directly; the Rust schema
change was verified statically (enum + description text match the asserted values
and the deserializer/handler exist) because `cargo` is not on this session's PATH.

## What was not checked

- The Rust test suite was **not compiled or run** here (`cargo`/`rustc` absent
  from PATH; `scripts/dev_cargo.sh` not invoked). The two added assertions in
  `communicate_tests.rs` were verified only by static comparison against
  `communicate.rs`. Recommend a routine `cargo test -p jcode-app-core --lib
  schema_advertises_supported_swarm_fields` in a build-capable session; this is a
  low-risk confirmation, not a blocker.
- No live swarm/daemon execution, provider calls, network, packaging, updater, or
  platform-gated work was performed (correctly out of scope and gated).
- Runtime-truth revalidation of `BASELINE.md` facts (runtime label/SHA, selfdev
  canary/pending, worktree count) was not re-run against the live system beyond
  confirming the documented derivation and the protected-hash match.

## Non-blocking observation (MINOR, informational only)

`BASELINE.md:29-34` tells a fresh session to derive the authority commit with
`git log -1 --format='%H' -- docs/fork/ideal-base/WORK_GRAPH.json`. Because the
ideal-base tree is still **untracked**, that command currently prints nothing.
This is expected for an uncommitted worktree and is fully mitigated: the seed
commit is stated literally at `BASELINE.md:16` and the bootstrap uses
`git rev-parse HEAD`. It resolves automatically once these files are committed.
No action required for PASS.
