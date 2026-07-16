# N1 curated integration stack plan

Authored before integration, as required by D2. Base: `main`
(`6ca1fcf2ec2366c7abc99664a485c40d60cec80e`). Source of truth: the recovery
line archived at `refs/archive/recovery/2026-07-15`
(`07c6bb585d57763a2d7e1c436f5b9b73bf71c6f5`, whose product tree equals accepted
source head `51168d16e9c708ae4afff09a6fc6402642d17782`; commits after it are
documentation only, verified by
`git diff 51168d16e9 recovery/2026-07-15 -- . ':(exclude)docs/fork'` being
empty).

Integration branch: `normalize/integration` built in dedicated worktree
`/Users/jrudnik/labs/jcode-normalization-integration`. Cleanup owner: this
normalization program; the worktree is removed in N5 after `main` promotion.

## Method

Product history is carried by cherry-picking the exact recovery product
commits in topological order, grouped into logical curated commits
(`git cherry-pick -n` per group, one commit per group). Every curated commit
message carries `Recovered-from:` trailers listing its source SHAs, keeping
provenance without interleaving evidence churn. The `docs/fork` recovery and
normalization record is imported afterward as dedicated documentation commits.

The reverted pair `a0f52cc74` + `d467ccdf9` (apply + revert of
"lifecycle handoff outcomes") is skipped; its re-application `8676e4f8a` is
picked. This is an enumerated intentional history difference with zero tree
difference. The four recovery merge commits are not replayed; their reachable
side-branch commits are picked linearly. Tree equivalence at the top of the
stack is the correctness proof for this flattening.

## Curated commit groups (topological, single purpose each)

| # | Curated commit | Source commits (topo order) | Validation |
|---|---|---|---|
| G1 | `chore(gates): shared rust production classifier and ratchet corrections` | `fb1168a6a`, `0508e3f7b`, `0674fe53d`, `f9c70d1be` | boundary B1 |
| G2 | `fix(r04): marker durability, persist terminal state before cleanup` | `a371fe758`, `eab42e1b5` | boundary B2 |
| G3 | `fix(r12): persist and stabilize terminal provider error evidence` | `a4d673ffd`, `8bb7afc16`, `2ef1041f9`, `8ac1c0f55` | boundary B2 |
| G4 | `fix(r02): fail-closed subscription tier handling` | `3063fe0fa`, `6396c429a`, `3aa644624`, `285f7ac79` | boundary B2 |
| G5 | `fix(identity): exact runtime identity projection and subscribe preflight` | `615ab1d9a`, `2010b53c8`, `d5e3fc7ef`, `0dd9efc13` | boundary B2 |
| G6 | `test(pilot): offline recovery validation driver and bounded pilot fixtures` | `526d96818`, `f6ca30c1a`, `f796ace46`, `86d3e3214`, `b1f5d187d`, `505cd8672` | boundary B2 |
| G7 | `fix(r12): close cancellation/retry/abandoned provider evidence classes` | `0e8ffa196`, `bd9ece15f`, `2aa53f6ed`, `d2eb0b379` | boundary B3 |
| G8 | `docs(product): record fork product governance proposal` | `a8a61653d` | none (docs + 1 proposal file) |
| G9 | `fix(r05b): swarm spawn/reclaim safety without protocol widening` | `2d36b9f49`, `a87f81f9d`, `282bad941`, `5ae37a297`, `c82de8b3f`, `da8fb9e01`, `2a5beea61`, `6115daa39`, `6dfe2cdb6`, `f13620596` | boundary B3 |
| G10 | `fix(r04): explicit lifecycle handoff and disconnect cleanup outcomes` | `8676e4f8a`, `8328e89b6`, `b1ea3108f`, `47f6d7dd8`, `24fb43188`, `2983bd437`, `8ac627db5`, `ce1ca968a`, `d54ba1b6d`, `221a94744` (skips `a0f52cc74`+`d467ccdf9` revert pair) | boundary B4 |
| G11 | `fix(r10): fail-closed draft-only release acquisition` | `c1bf53076`, `a62516b6f`, `9203aaf97`, `09e23e998`, `9e981514b` | boundary B4 |
| G12 | `fix(w5): fail-closed onboarding import consent` | `6cf11d4e6`, `1a65e6e54`, `47d92aeb6`, `cd9a8ae7b`, `4509bcd0a`, `b3b010316`, `95861f4f5` | boundary B4 |
| G13 | `docs(fork): import recovery forensic record and normalization authority` | tree import of `docs/fork` at `recovery/2026-07-15` | tree equivalence |

W7a-W7d then land on top of this stack in N2 as separately reviewed commits.
Normalization evidence commits (`docs(evidence)`) sit at the top of the stack.

## Validation boundaries (explicitly documented inseparable groups)

- B1 after G1: `python3 -m unittest discover -s tests -p
  'test_rust_production_filter.py'` plus `python3 -m py_compile` of the four
  gate scripts. G1 is self-contained tooling.
- B2 after G6: `cargo test -p jcode-app-core --lib` and
  `cargo test -p jcode-base --lib`. G2-G6 are the interleaved recovery
  prerequisites (marker, evidence, tier, identity, pilot fixtures); they share
  touched files (`client_lifecycle.rs`, `evidence.rs`, `handshake.rs`) and are
  validated as one documented group at this boundary.
- B3 after G9: `cargo test -p jcode-app-core --lib` (swarm/comm/evidence
  focused modules re-run within it).
- B4 after G12: full `cargo test --workspace` plus the B1 python suite; this is
  the product-stack top boundary.
- After G13: tree equivalence proof:
  `git diff normalize/integration recovery/2026-07-15` must be empty except
  the enumerated intentional differences below.

Expected-red R09 ratchets (panic 48, swallowed 3074, production-size,
test-size at recovery HEAD semantics) are not run per boundary; they are
re-proven once over the finished stack in N2 with no `--update`.

## Enumerated intentional differences (curated vs recovery tree)

1. None in product paths. The product tree must be byte-identical.
2. `docs/fork` differences only if produced by normalization program commits
   made after the archive ref (each enumerated in the equivalence evidence).
3. History-shape differences (flattened merges, skipped revert pair, grouping)
   are history-only and carry zero tree delta.

## Failure handling

Any cherry-pick conflict is resolved only toward the recovery tree state and
re-verified with `git diff <group paths> recovery/2026-07-15` at the group
boundary. Any boundary test failure stops the stack; the failing state is
preserved append-only and the plan is amended rather than silently reordered.
