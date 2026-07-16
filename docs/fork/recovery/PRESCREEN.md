# Mechanical responsibility pre-screen

Status: Phase 0 evidence, not an ownership decision

Measured at `2026-07-15T06:50:07Z` from fork
`d756d6a2c26fa63c3b89abe1bd29f8ff41c516dd`, upstream
`802f6909825809e882d9c2d575b7e478dce57d3b`, and merge base
`631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`.

This pre-screen ranks where independent review is likely to pay off. It does
not choose authority, disposition, or final responsibility boundaries. The
Mapper and Map critic must challenge the seed boundaries before review modes
become authoritative.

## Method and limits

The pre-screen used four mechanical signals:

1. Fork, upstream, and overlapping changed paths since the merge base.
2. Non-merge commits touching responsibility-specific path keywords.
3. Changed hunk contexts from Rust, Python, and shell diffs as an approximate
   symbol signal.
4. Stable patch IDs for every non-merge commit on both sides.

Path matching was intentionally non-exclusive. A protocol file used by session
resume can count in both R03 and R04. Counts across rows therefore do not sum
to the repository totals. Broad deletion and curated-sync commits can dominate
path counts, so commit counts are triage signals rather than authorship or
semantic-equivalence proof.

The seed classifier left 127 fork-changed and 64 upstream-changed paths
unclassified. That is an explicit input to the Mapper, not evidence that those
paths are irrelevant.

Reproduce the raw change sets:

```bash
fork=d756d6a2c26fa63c3b89abe1bd29f8ff41c516dd
up=802f6909825809e882d9c2d575b7e478dce57d3b
base=$(git merge-base "$fork" "$up")
git diff --name-only "$base..$fork"
git diff --name-only "$base..$up"
git diff --unified=0 "$base..$fork" -- '*.rs' '*.py' '*.sh'
git diff --unified=0 "$base..$up" -- '*.rs' '*.py' '*.sh'
```

The path match signals were:

| ID | Mechanical match signals |
|---|---|
| R00 | fork docs/scripts, `.rerere-cache`, `.fork.toml` |
| R01 | selfdev, reload, build identity/hash/registry, server util/runtime/socket/spawn |
| R02 | config, provider, auth, account, credential, pricing, sidecar, route, model, gateway |
| R03 | protocol, wire, gateway, client lifecycle, server events, remote, handshake, reconnect, iOS |
| R04 | session, supervision, recovery, backoff, shutdown, cancel, process markers |
| R05 | swarm, communicate/comm, `jcode-plan`, DAG, scheduling, run-plan, task graph |
| R06 | persistence, storage, journal, snapshot, backup, memory, control log, history, replay |
| R07 | tools, MCP, discovery, telemetry, browser/computer, network, consent, analytics |
| R08 | TUI, CLI, desktop, keymap, render, UI |
| R09 | workflows, tests, check scripts, budgets, benchmarks, fuzz, clippy, formatting |
| R10 | Nix/flake, Cargo manifests, release, updater, installer, package, distro |
| R11 | docs, backlog, maintenance, README, changelog, AGENTS |

## Divergence measurements

`F/U/O` means fork-changed, upstream-changed, and changed on both sides.
`Commits` and `hunks` are fork/upstream counts.

| ID | Files F/U/O | Commits | Hunks | Mechanical observation |
|---|---:|---:|---:|---|
| R00 | 215/0/0 | 30/0 | 4/0 | Fork-only governance machinery is large, including 199 rerere paths, while the visibility ref remains stale. |
| R01 | 29/17/17 | 30/27 | 142/84 | Every upstream-matched path also changed in the fork; reload/runtime symbols diverged on both sides. |
| R02 | 90/58/55 | 37/44 | 671/286 | Heavy two-sided config/provider/auth divergence; this remains the leading pilot candidate. |
| R03 | 53/35/35 | 33/38 | 309/116 | All upstream-matched protocol paths overlap, including wire, gateway, reconnect, and client lifecycle. |
| R04 | 49/28/27 | 29/21 | 260/141 | Session resume, recovery, and supervision have near-total upstream overlap. |
| R05 | 74/26/26 | 68/23 | 688/188 | Fork orchestration grew substantially while every upstream-matched path also changed locally. |
| R06 | 28/18/14 | 26/14 | 173/113 | Persistence overlap is smaller but includes swarm state, replay, session history, and memory. |
| R07 | 69/52/52 | 42/56 | 343/241 | Tool/discovery/MCP overlap is complete for upstream-matched paths and has substantial upstream activity. |
| R08 | 197/166/160 | 53/134 | 1150/1046 | Largest shared behavioral surface; too broad to review coherently without Mapper splits. |
| R09 | 138/98/92 | 97/132 | 686/336 | Tests and gates are highly divergent; current gate trust is mixed and blocks architecture work. |
| R10 | 15/8/6 | 31/24 | 8/16 | Narrower code surface, but authority is split across Nix, updater, release, and fork branches. |
| R11 | 116/25/21 | 111/33 | 0/0 | Fork governance volume is high and several external maintenance statuses were stale. |

Repository totals at this checkpoint were 934 fork-changed files, 425
upstream-changed files, and 406 overlapping files.

## Patch-equivalence pre-screen

No exact stable patch-ID cluster was shared between the 288 fork-side and 243
upstream-side non-merge commits in these ranges. The exact commands were:

```bash
fork=d756d6a2c26fa63c3b89abe1bd29f8ff41c516dd
up=802f6909825809e882d9c2d575b7e478dce57d3b
base=$(git merge-base "$fork" "$up")
git log --no-merges --pretty=format:'commit %H' -p "$base..$fork" \
  | git patch-id --stable > /tmp/fork.patch-ids
git log --no-merges --pretty=format:'commit %H' -p "$base..$up" \
  | git patch-id --stable > /tmp/upstream.patch-ids
comm -12 \
  <(cut -d' ' -f1 /tmp/fork.patch-ids | sort -u) \
  <(cut -d' ' -f1 /tmp/upstream.patch-ids | sort -u)
```

The empty result does **not** mean upstream behavior is absent. The
single-parent curated sync combined many changes into `b3ed82a6b`, so exact
per-commit patch identity is unlikely. Full seam reviews must search semantic
and symbol-level equivalence and record their assumptions.

## Operational evidence and maintenance reconciliation

The following external records are summarized here so the repository, rather
than an untracked note, carries the decisive facts.

| Applies to | Source and SHA-256 | Source-backed summary |
|---|---|---|
| R05 | `/Users/jrudnik/notes/projects/jcode/maintenance/bug-run-plan-spawn-storm.md`; `7fdd90404a8cfb7e729686621df072ad13a48d48e0e88e5310926552d75eb992` | On 2026-07-07 a six-node run-plan completed zero nodes, emitted about 76 assignments in two minutes, and created 190 session files. Terminal-backed workers died before prompts; explicit headless spawns succeeded. Missing failure backoff and spawn-mode authority were implicated. |
| R01 | `/Users/jrudnik/notes/projects/jcode/maintenance/bug-server-reload-stale-daemon-version-check.md`; `80012e2ce61c578c943263b944bbaca27ac7dbd440af50c1f21f6e0291d8f1a9` | On 2026-07-14 the Nix binary, selfdev current build, stable channel, and live daemon diverged. Reload reported “already newest” while the daemon still mapped an old executable; forced reload changed the mapped build while preserving sessions. |
| R09/R11 | `/Users/jrudnik/notes/projects/jcode/maintenance/audit-2026-07-14-fork-health.md`; `a672073ed15ece35aee1f5dd7c10b85b61a49e96ce4d656cda0947faa095ba64` | The audit found panic/swallowed-error false positives from brace counting and exact-only `cfg(test)` matching, while size-ratchet failures were structurally real. It also warned that old hot-path stashes overlap fixes already integrated. |

Git ancestry verification corrected the stale maintenance queue:

- `a69ef9710`, `3d80eaf34`, `97529fd6c`, `2ccf43fd7`, `17cceb1a1`,
  and `e6ff371c1` are ancestors of the recovery branch.
- The old `fix-config-hotpath-spam` and `fix-marker-sweep` task-graph entries
  therefore do not represent open implementation work.
- `agent/marker-hardening` remains preserved at non-ancestor commit `6fc5623a5`;
  the integrated lineage contains `97529fd6c` with the same subject and change
  shape. No branch was deleted.
- The three hot-path stashes remain preserved and must not be popped. Their
  touched areas overlap the already-integrated config and sidecar fixes; no
  patch-equivalence claim is needed to establish that replaying them is unsafe.

## Risk ranking

Scores range from 0 to 4 for divergence, operational risk, contested
ownership, and pilot dependency. They are triage inputs, not review verdicts.

| Rank | ID | Div | Ops | Contest | Pilot | Total | Provisional consequence |
|---:|---|---:|---:|---:|---:|---:|---|
| 1 | R00 | 4 | 4 | 4 | 4 | 16 | Full-review candidate; governs every later sync claim. |
| 1 | R01 | 4 | 4 | 4 | 4 | 16 | Full-review candidate; protected live-build authority invariant. |
| 1 | R02 | 4 | 4 | 4 | 4 | 16 | Full-review and pilot candidate. |
| 4 | R05 | 4 | 4 | 4 | 3 | 15 | Full-review candidate; quantified operational incident. |
| 4 | R09 | 4 | 4 | 3 | 4 | 15 | Full-review candidate; truth-gate dependency. |
| 6 | R03 | 3 | 4 | 3 | 3 | 13 | Sixth full-review candidate unless mapping exposes a narrower composition seam. |
| 7 | R04 | 3 | 4 | 3 | 2 | 12 | First escalation candidate if mapping or pilot crosses session invariants. |
| 8 | R06 | 3 | 3 | 3 | 2 | 11 | Likely light initially, with escalation on persistence coupling. |
| 8 | R07 | 3 | 3 | 3 | 2 | 11 | Must be split if discovery, MCP, telemetry, and network policy have different authorities. |
| 8 | R08 | 4 | 3 | 3 | 1 | 11 | Mapper must split this broad surface before assigning review depth. |
| 11 | R11 | 3 | 2 | 3 | 2 | 10 | Light governance review after stale-state reconciliation. |
| 12 | R10 | 2 | 2 | 2 | 1 | 7 | Likely light or defer pending runtime-authority decisions. |

This provisional top six is R00, R01, R02, R03, R05, and R09. Phase 1 may
replace a candidate, but it may not silently expand beyond six full reviews.

## Pilot prerequisites suggested by the pre-screen

If provider/config/routing remains the pilot, the minimum ledger prerequisites
are R00 sync governance, R01 live runtime authority, R02 provider/config
authority, the protocol slice of R03 that exposes runtime/model identity, and
R09 trusted baseline tests. R05 is not a provider pilot prerequisite unless
the pilot uses swarm-driven execution.

## Explicit unknowns for Phase 1

1. Which 127 fork and 64 upstream unclassified paths represent missing
   responsibilities rather than incidental files?
2. Which behaviors inside `b3ed82a6b` are semantically equivalent to upstream
   commits despite zero exact patch-ID matches?
3. Should R08 split into input/command semantics, render state, session picker,
   and desktop adaptation before review depth is assigned?
4. Should R07 split tool execution authority from MCP lifecycle, discovery,
   telemetry, and network consent?
5. Is protocol/runtime identity in R03 a standalone seam or part of R01?
6. Which size-ratchet regressions are inherited from the curated sync, which
   are fork-owned, and which are mixed?
7. Does the current live daemon still reproduce the external stale-build
   incident, or has later code changed the failure mode without closing the
   authority gap?
8. What exact provider/config stack offers the cheapest pilot while exercising
   representative fork and upstream divergence?
9. Which existing tests are trusted enough to define the pilot regression
   budget before any replay or composition attempt?

