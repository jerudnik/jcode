# NEXT-SESSION KICKSTART PROMPT
# (paste the block under "PROMPT" as the first message of a fresh jcode session,
#  started in ~/infrastructure/jcode with the Serena MCP server available)

------------------------------------------------------------------------
PROMPT:
------------------------------------------------------------------------
You are picking up a multi-session design thread about making a fast-moving
upstream fork + Nix packaging + self-developed features SUSTAINABLE. Work in
~/infrastructure/jcode.

## Step 0 - Bake in the initiative FIRST (do this before anything else)
Create a durable todo/plan for THIS session with these phases, and keep it in
sync as you go:
  1. Consume the self-destruct memory.
  2. Synthesize the coherent linear report.
  3. Map the workflow out (diagram + concrete file/branch/flake layout).
  4. Finalize the draft WITH the human (stop and review before deleting anything).
  5. ONLY after the report is finalized and saved: run a WIDE exploration of prior
     art + alternatives + sidestep-the-whole-problem reframes (details in Step 5).
  6. Delete the self-destruct memory once the report supersedes it.

## Step 1 - Consume the memory (Serena MCP)
The thinking from the prior conversation is captured in a Serena memory on the
`jcode` project (registered in ~/.serena; in-repo at .serena/memories/):
  - memory name: `fork-divergence-thinking-selfdestruct`
Read it with the Serena MCP tool `read_memory` (or, from a shell as a fallback:
`uvx --from git+https://github.com/oraios/serena serena memories read \
   fork-divergence-thinking-selfdestruct /home/john/infrastructure/jcode`).
Also read the companion already on disk:
  - docs/architecture/SELFDEV_NIX_DAEMON_DIVERGENCE.md  (the NS1..NS5 research)
  - docs/fork/patch-ledger.md, docs/BRANCHING.md, .fork.toml, scripts/sync-local.sh
Do NOT delete the memory yet. It self-destructs only after the report exists
(Step 6).

## Step 2 - Synthesize the coherent, linear report
Write ONE prose-first, top-to-bottom report (no scattered notes) that a reader
can follow start to finish. Save it to:
  docs/architecture/FORK_SUSTAINABILITY_MODEL.md
It must cover, in order:
  - The problem (fast/major upstream churn; fork must reconcile upstream+nix as a
    flake input; PLUS large NON-upstreamable personal feature/experiment work;
    dogfood-live-with-nix-as-safe-fallback).
  - The two hard constraints: (a) repo-containment (jcode owns everything thick;
    4nix owns one line) and (b) compute frugality (the cost ladder: cargo check ->
    cargo test -> selfdev build+reload -> nix build cached variant).
  - Ground-truth divergence facts (the cherry-mark illusion: 8 real local commits
    not 35; 44 additive files vs 60 invasive edits = the real conflict surface;
    the existing rails + sync-local.sh; the patch-ledger as embryo).
  - The tiered model: BUILD (crane base, cached cargoArtifacts) / COMPOSE (thin,
    non-forking overlay+HM+wrapper) / FEATURE-EXPERIMENT (nix overrideAttrs
    base->base' variants: pinned, composable, disposable, own store path, own
    cached check) / VALIDATION (per-feature nix checks = executable patch-ledger
    rows). Map which artifact goes in which tier and WHY each constraint picks it.
  - The "new VCS model" framing (agent-maintained patch stack + machine-checkable
    patch metadata + automated semantic validation as the merge gate; the drive-by
    -PR parallel). Mention Jujutsu (jj) as an option for the restacked stack.
  - A recommended SEQUENCE (NS4 provenance-stamp first = cheapest+most legible;
    then one nix proof-slice; then drive the 60-file conflict surface toward zero).
  - The open questions (where the 8 commits live: main vs stack/exp; forgejo role;
    jj vs git; minimal {patch,check,pin} triple so adding a feature is one step).
Keep the human's 4 goals as explicit success criteria the model must satisfy:
generalize, dead-easy, idiot-proof, transparent.

## Step 3 - Map the workflow
Add to the report: a mermaid diagram of the end-to-end flow (upstream -> CI rails
-> nix base -> compose tier -> feature variants -> 4nix consumer + dogfood + safe
fallback), AND a concrete proposed layout: exact branch names, exact flake output
names, exact directory layout for features (e.g. nix/features/<name>/{patch,
check.nix,pin}), and the one-step "add a feature" recipe.

## Step 4 - Finalize WITH the human
Present the draft, ask the open questions, and revise until John signs off. Do not
proceed to Step 5 or delete the memory until he says the draft is final.

## Step 5 - Wide exploration AFTER finalize (the baked-in initiative)
Once the draft is finalized, run a broad, skeptical search (use parallel-cli
research + DeepWiki via webfetch on relevant repos + targeted web search). Cover
THREE buckets, each finding labeled PROMISING / TRIED-AND-FAILS / UNKNOWN + source:
  A. SOLUTIONS/TOOLS for carrying a large downstream patch stack on a fast-moving
     upstream: quilt, stgit, git-series, "patch queue" managers, Jujutsu (jj)
     workflows, Gentoo/Nixpkgs/Debian patch-set maintenance, Android/AOSP
     `repo` + topic branches, Chromium/Firefox downstream rebase machinery,
     git-imerge, `git rerere`, rebase automation, Mercurial MQ/evolve.
  B. ERGONOMIC WORKFLOWS / PROSTHETICS: overlay/extension architectures that avoid
     editing upstream (plugin systems, trait-seam extension points, "soft fork",
     Nix overlays/overrideAttrs as a patch manager, out-of-tree kernel modules,
     VS Code/Neovim extension models), and agent-maintained-merge approaches.
  C. SIDESTEP-THE-WHOLE-PROBLEM REFRAMES (explicitly consider that John may have
     over-complicated it): just track upstream and contribute everything; just
     hard-fork and stop pulling; vendor a pinned snapshot and only bump
     deliberately; build features as a SEPARATE tool that consumes jcode as a
     library/over its protocol rather than editing it; or don't fork at all and
     use jcode's own extension surfaces (config/overlays/tools/MCP). For each
     reframe, state what John would GAIN and what he'd LOSE.
Write the exploration as docs/architecture/FORK_SUSTAINABILITY_PRIOR_ART.md and
fold a short "decision" section into the main report.

## Step 6 - Self-destruct
After the report (Steps 2-4) is finalized and saved, DELETE the memory via the
Serena `delete_memory` tool (name `fork-divergence-thinking-selfdestruct`), or
`uvx --from git+https://github.com/oraios/serena serena memories delete \
   fork-divergence-thinking-selfdestruct /home/john/infrastructure/jcode`.
Confirm deletion in your final summary.

## Constraints (carry these the whole session)
- SSH-sign commits (verify %G? = G; key is John's FIDO2 hardware key, must be
  plugged in). Commit as you go, your changes only, stage by name.
- Repo-containment: propose mechanics INSIDE the jcode repo, not 4nix.
- Compute-frugal: prefer cargo check / targeted tests; do not do heavy nix/release
  builds just to answer cheap questions. Run long builds in the background.
- This is DESIGN + a possible smallest prototype (NS4). Do not implement the whole
  model in one session; finalize the report and the decomposition first.
------------------------------------------------------------------------

## Operator notes (for John, not part of the prompt)
- The Serena MCP server is already kickstarted/cached via uvx and the `jcode`
  project + memory are registered (verified: `serena memories list` shows
  `fork-divergence-thinking-selfdestruct`).
- If the Serena MCP tool isn't wired into the session, the shell fallbacks above
  work (`uvx --from git+https://github.com/oraios/serena serena memories ...`).
- The memory is intentionally machine-local (.serena/ is gitignored), so it won't
  pollute the tracked tree or get committed.
