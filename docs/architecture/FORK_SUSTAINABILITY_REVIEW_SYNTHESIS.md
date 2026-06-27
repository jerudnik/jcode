# Fork Sustainability Review Synthesis

Date: 2026-06-27
Status: synthesis for the cut-down `FORK_SUSTAINABILITY_MODEL.md`.

## The cut

The earlier draft modelled jcode as "a large downstream patch stack on a fast
upstream" and proposed tiers (BUILD/COMPOSE/FEATURE/VALIDATION), Nix feature
variants/stacks, named daemon instances, and an executable patch ledger.

The repo's own numbers reject that framing:

- 30 feature commits on `main`, 9 packaging commits. Small.
- 107 files touched = 47 brand-new files + 60 edits, and most edits are
  **pure insertions** (0 deletions).
- Only **7 source files** actually rewrite >5 upstream lines.
- `git cherry` proved the "35 ahead" was mostly already-upstream commits under new
  hashes (6h CI rebase rewrites history).

That is a routine rebase with a few recurring conflict points, not a patch-queue
problem. So the model collapses to what git and the existing flake already do.

## The target model

> Main is the fork. CI rebases it on upstream. `rerere` remembers conflict fixes.
> New features add files, they don't edit upstream. `doctor` names the running
> binary. That's it.

## What to actually build (cheap, ordered)

1. **`git rerere` (shipped)** in the repo and the CI rebase job, with the
   `rr-cache` shared through a tracked `.rerere-cache/` (`scripts/rerere-cache.sh`
   + `scripts/rerere-rebase.sh`). Recurring conflicts in the 7 files get resolved
   once, then replay automatically on every 6h rebase; a new conflict fails CI
   loudly. This was the single highest-leverage change.
2. **`jcode doctor`** binary-identity view: client/server path, origin
   (nix/selfdev/source), commit, dirty, a compat verdict, and the fallback
   command. `jcode-build-meta` already holds the data; just surface it.
3. **Shrink the 7 rewrite-files** to additive seams (new file + one registration
   line), upstreaming each seam so the conflict disappears for good.
4. Keep `patch-ledger.md` as a plain-doc index. Nothing "executable".

## What is deferred or rejected

- **Rejected:** Nix feature variants/stacks, `nix/features/<name>/{patch,check.nix}`,
  quilt/StGit/topgit patch queues, named daemon instances, a compat-negotiation
  framework. All invent structure for a problem `git rebase + rerere + additive
  seams` already solves. (`overrideAttrs.patches` is a one-liner if a separate
  binary identity is ever truly needed.)
- **Deferred:** Jujutsu (jj). Its strength is large reorderable stacks; revisit
  only if rerere + seams stop coping.

## Feature placement rule

- Needs to run inside the agent loop -> additive code on `main`.
- Can be a tool/setting -> external MCP/ACP tool or config (`.jcode/mcp.json`,
  skills, MCP servers). Zero divergence.
- Is really an extension point -> upstream it.

## Slogan

> `main` rebased by CI, `rerere` for the repeats, additions over edits, `doctor`
> for identity, Nix for the fallback. No tiers.
