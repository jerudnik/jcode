# Graph-first execution protocol

This protocol makes the ideal-base program resumable, delegable, and auditable.
The repository and accepted node artifacts are the shared medium. Chat and agent
memory are caches.

## 1. Bootstrap before mutation

Run:

```bash
python3 scripts/ideal_base_railway.py check
python3 scripts/ideal_base_railway.py status
python3 scripts/ideal_base_railway.py next --json
```

Also record `git status --short`, `git rev-parse HEAD`, worktree count, runtime
channel identity, selfdev pending/canary state, and the protected prompt hash.
Unexpected drift blocks mutation until classified.

## 2. Seed work through the native graph

Use `swarm task_graph` with `mode: "deep"` and the runnable nodes printed by the
validator. The static graph is deliberately a seed. A worker assigned a composite
workstream must expand its node using the prescribed children in
`WORK_GRAPH.json`, then may add new children when source inspection or a gate finds
a real gap.

The graph is authoritative while the coordinator session is alive. `STATE.json`
is the durable restart checkpoint. If the live graph is lost, re-run `next --json`
and seed only nodes whose dependencies are accepted.

## 3. Agent roles and model routing

- Exploration, architecture, and debugging: Fable-class design agent.
- Implementation and process-running validation: GPT 5.5-class implementation agent.
- Verification, critique, and final signoff: Opus-class reviewer.
- Bulk context extraction: low-effort context worker.

A worker that needs more than two or three subtasks expands its graph node or
spawns one manager for its subtree. Do not fan out unbounded ad hoc work outside
the graph.

## 4. File ownership and concurrency

`WORK_GRAPH.json` declares `owned_paths` for each node.

- Only one active implementation node may own a path at a time.
- Read-only reviewers may inspect any path but write only under their assigned
  `evidence/<node-id>/` or `reviews/` path.
- The coordinator alone updates `STATE.json` and [`DECISIONS.md`](DECISIONS.md).
  These paths are reserved by `coordinator_owned_paths` in `WORK_GRAPH.json` and
  are never worker-owned. W0.3 and S03 artifacts propose the checkpoint content;
  the coordinator performs the atomic write.
- Workers commit only their exact paths. They do not stage unrelated concurrent
  changes.
- If an unexpected overlap appears, pause one node and resolve it with a direct
  message. Do not rely on last-write-wins or broad resets.

## 5. Node lifecycle

The durable states are:

`pending -> in_progress -> implemented -> verifying -> accepted`

Alternative terminal dispositions are `authorization_blocked`, `superseded`, or
`rejected`. A `blocked` node is non-terminal and must name the missing evidence and
next action.

Implementation is not acceptance. A separate verify node or independent reviewer
must execute the declared gates. Verification failure injects a fix node, then
re-runs the same verification node.

## 6. Required handoff artifact

Every `complete_node` call in deep mode must provide:

```json
{
  "findings": "What changed or was established, including dependent node IDs.",
  "evidence": ["file:line, commit, and evidence-directory references"],
  "edge_cases_considered": [
    "normal, failure, concurrency, crash, restart, and cleanup cases"
  ],
  "validation": "Exact commands, counts, outcomes, and residue checks.",
  "open_questions": [],
  "confidence": "high",
  "what_i_did_not_check": [
    "Live provider and platform-gated validation was outside this node."
  ]
}
```

Use `medium` or `low` honestly. `what_i_did_not_check` must be non-empty and
specific. A low-confidence artifact requires a follow-up gap node before a sibling
gate can pass.

## 7. Evidence contract

Each accepted implementation or verification node writes bounded evidence under:

```text
docs/fork/ideal-base/evidence/<node-id>/
```

The directory contains a short `README.md`, command log or machine-readable result,
and `SHA256SUMS` when more than one evidence file is retained. Avoid committing
large rebuildable output. Reference sealed external evidence by path and checksum
rather than copying it.

Independent reviews go under `reviews/` and name the exact commit reviewed.
Historical evidence remains under the frozen recovery/normalization roots.

## 8. Checkpoint and commit discipline

After a node survives verification:

1. Commit its bounded implementation and tests.
2. Commit bounded evidence/review updates when appropriate.
3. Run `ideal_base_railway.py checkpoint` with the accepted commit and evidence.
4. Commit the coordinator-owned `STATE.json` checkpoint separately or with the
   bounded evidence commit.
5. Re-run `check`, `status`, and `next --json`.

Do not mark a node accepted before its commit and evidence exist.

## 9. Recovery after interruption

A fresh coordinator must not infer progress from chat. It must:

1. Validate the repository and protected hash.
2. Read `STATE.json` and verify every accepted node's commit is reachable.
3. Verify referenced evidence exists.
4. Reclassify any `in_progress`, `implemented`, or `verifying` node whose owner is
   gone as `blocked` until its diff and test state are inspected.
5. Seed only currently runnable nodes.
6. Preserve failed attempts and inject repair nodes rather than rewriting history.

## 10. Authorization boundaries

Stop and request authorization before provider requests, credential use, release
publication, pushes, Apple signing/device work, destructive archive or ref cleanup,
or changes to protected historical evidence. Deterministic mocks, local servers,
disposable homes, private sockets, and local builds may proceed autonomously.
