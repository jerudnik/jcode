# Post-recovery normalization baseline

Observed 2026-07-16 after Phase 6 closure and before normalization work. Every
fact must be revalidated at the start of each session. This file cannot contain
the hash of the commit that contains itself. The durable authority commit is
therefore identified with:

```bash
git log -1 --format='%H' -- docs/fork/normalization/COMPLETION_STANDARD.md
```

The pre-infrastructure parent is fixed below. After this directory is committed,
the expected worktree state is that the preserved recovery prompt is the sole
dirty path. The authority commit itself is not drift.

## Canonical checkout candidate

| Fact | Value |
|---|---|
| Primary path | `/Users/jrudnik/labs/jcode` |
| Current branch | `recovery/2026-07-15` |
| Pre-normalization-infrastructure head | `cdc2cc2b4cea51c185de330c8e15e08615acc46c` |
| Phase 6 closure commit | `ba45f20aa61fdf597bbe4a1d11e94d1dd43c8c38` |
| Accepted recovery source head | `51168d16e9c708ae4afff09a6fc6402642d17782` |
| `main` | `6ca1fcf2ec2366c7abc99664a485c40d60cec80e` |
| `main...recovery` at snapshot | `0` commits unique to `main`, `166` unique to recovery |
| Merge base | `6ca1fcf2ec2366c7abc99664a485c40d60cec80e` (`main` is an ancestor) |
| `vendor/upstream` | `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Expected post-infrastructure status | Only `docs/fork/recovery/ORCHESTRATOR_PROMPT.md` modified |
| Preserved prompt diff SHA-256 | `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00` |
| Local branches | 40 |
| Tags | 138 |
| Commits reachable from local refs but not recovery head | 916 |
| Stashes | Exactly 4 |

No non-document path changed after accepted source head `51168d16e9` at this
snapshot.

## Stash identity and preservation hazard

Only `stash@{0}` is named by the real ref `refs/stash`. Older entries are reflog-
only. A normal `git bundle create ... --all` does **not** preserve `stash@{1..3}`.
All four stash commits and their index parents must be physically preserved in a
separate verified stash-object bundle before any stash drop, reflog expiration,
garbage collection, branch deletion, or worktree cleanup.

The safe archive pattern is to give every reflog-only object a durable temporary
ref before bundling. `$ROLLBACK_DIR` must be outside any worktree and included in
the cleanup manifest:

```bash
for i in 0 1 2 3; do
  git update-ref "refs/archive/normalization/stashes/$i/worktree" \
    "$(git rev-parse "stash@{$i}")"
  git update-ref "refs/archive/normalization/stashes/$i/index" \
    "$(git rev-parse "stash@{$i}^2")"
done
git bundle create "$ROLLBACK_DIR/jcode-stashes.bundle" \
  $(git for-each-ref --format='%(refname)' \
    refs/archive/normalization/stashes/)
git bundle create "$ROLLBACK_DIR/jcode-all-refs.bundle" --all
git bundle verify "$ROLLBACK_DIR/jcode-stashes.bundle"
git bundle verify "$ROLLBACK_DIR/jcode-all-refs.bundle"
# In the disposable restore, verify the -u stash payload too:
: "${RESTORE_REPO:?set RESTORE_REPO to the disposable restored repository}"
(
  cd "$RESTORE_REPO"
  git cat-file -e 7c68ef5f59359ed89e0979b99bba143c74d926aa^{commit}
)
```

Creating these additive archive refs is reversible. Their creation can change
the `git rev-list --all --not recovery/2026-07-15 --count` value from the initial
916 because reflog-only stash objects become reachable from refs. Record that
expected transition explicitly rather than treating it as unexplained drift.
Deleting the archive refs is not authorized until final cleanup approval and
successful restoration testing.

| Entry | Stash commit | Index parent | Untracked parent | Subject |
|---|---|---|---|---|
| `stash@{0}` | `1f54abc9fbb0190f59af2fe5744e8e8dfb99c67f` | `f8bb6829da55c4754eb42c01f28d47c12f1c881c` | none | `On main: WIP fix-config-hotpath-spam part 3 (scorpion): account_failover hot path` |
| `stash@{1}` | `975b91b8336122d55eb8d0955fb6aa09158e5b27` | `7385174012d5bf5dd8820d0cc7cd286e902cfa96` | none | `On main: WIP fix-config-hotpath-spam part 2 (scorpion): Config::load->config() TUI callers` |
| `stash@{2}` | `5dc53ed77b98effbd682402ddb10a6c6d6c286fe` | `608a17fbd08dad9c62d731b8d9edae65b5c1a4dc` | none | `On main: WIP fix-config-hotpath-spam (scorpion, stopped mid-work): config warn-once + sidecar log dedup` |
| `stash@{3}` | `29d49b250a6a7e924fa1beb33a07f635fc13c9be` | `78a8baeddb70ccdc989c1566e8bdf80ee582e3e3` | `7c68ef5f59359ed89e0979b99bba143c74d926aa` | `On main: wip before upstream sync` |

## Remotes

- `origin` and `github` both fetch from and push to
  `https://github.com/jerudnik/jcode.git`; their duplication needs an explicit
  retain/remove rationale.
- `upstream` fetches from `https://github.com/1jehuang/jcode.git` and has push
  disabled.
- No remote push is authorized by this baseline.

## Worktree topology

Git registers **29 worktrees** for the same repository object database:

- 25 paths under `/Users/jrudnik/labs`, including the primary checkout;
- 4 paths under `/private/tmp`;
- 28 auxiliary worktrees are clean;
- the primary checkout is expected to contain only the preserved prompt edit
  after the normalization authority files are committed.

The `jcode-*` directories under `/Users/jrudnik/labs` are Git worktrees, not
independent clones. Their `.git` files point into
`/Users/jrudnik/labs/jcode/.git/worktrees/`.

### Labs worktree groups

- prerequisite/fix lanes:
  `jcode-fix-r01-r03a-identity`, `jcode-fix-r02-tier`,
  `jcode-fix-r04-marker`, `jcode-fix-r12-evidence`;
- governance/pilot lanes:
  `jcode-governance-decisions`, `jcode-light-control`,
  `jcode-light-ledgers`, `jcode-light-pilot`,
  `jcode-pilot-prereq-ledgers`;
- restored orchestrator lanes:
  `jcode-orchestrator-s4`, `jcode-orchestrator-s5`,
  `jcode-orchestrator-s6`;
- seam-review lanes:
  `jcode-seam-r01`, `jcode-seam-r02`, `jcode-seam-r03a`,
  `jcode-seam-r04`, `jcode-seam-r05b`, `jcode-seam-r12`;
- implementation lanes:
  `jcode-w1-r12`, `jcode-w2-r05b`, `jcode-w3-r04`, `jcode-w4-r02`,
  `jcode-w5-consent`, `jcode-w6-r10`.

### Temporary worktrees

- `/private/tmp/jcode-hotpath`
- `/private/tmp/jcode-marker-hardening`
- `/private/tmp/jcode-recovery-gate-parser`
- `/private/tmp/jcode-up`

Several branch tips are not ancestors of the recovery head because work was
integrated through cherry-picks, replacements, or later corrected chains. There
are 916 commits reachable from local refs but not the recovery head. Worktree or
branch cleanup must preserve all refs first and must not infer that a non-
ancestor tip is disposable merely because Phase 6 closed.

## Current runtime and host facts

### Binaries on `PATH`

1. `/Users/jrudnik/.local/bin/jcode` points to
   `/Users/jrudnik/.jcode/builds/current/jcode`.
2. `/etc/profiles/per-user/jrudnik/bin/jcode` points into
   `/nix/store/w2wbi1jjm21hjqb9l920c5ph2m733g6n-home-manager-path/bin/jcode`.

The second entry is declaratively managed by home-manager. It is **not** an
agent-removable duplicate. Any change must be made through its declarative Nix
source with explicit user approval.

At the snapshot:

- `~/.jcode/builds/current/jcode` points to
  `~/.jcode/builds/versions/6c6a4f2c8-dirty-7b4ec829c656/jcode`, SHA-256
  `fd6297d9d9b135f7c8233dc27a6119bea767f74256e6dddccd1a0e5f557c6dd9`;
- `~/.jcode/builds/shared-server/jcode` points to
  `~/.jcode/builds/versions/65cfde463/jcode`, SHA-256
  `a4973e8ce3551df3717af77007aade88e072d24683d6e34bae3bec6072d8b733`.

These exact pointer targets and hashes are the pre-promotion binary rollback
anchors. Repointing either link or replacing either target requires a dry-run
restore command and explicit approval.

The exact link restoration core, to be printed and dry-run checked before any
promotion but executed only with approval or during an actual rollback, is:

```bash
ln -sfn \
  /Users/jrudnik/.jcode/builds/versions/6c6a4f2c8-dirty-7b4ec829c656/jcode \
  /Users/jrudnik/.jcode/builds/current/jcode
ln -sfn \
  /Users/jrudnik/.jcode/builds/versions/65cfde463/jcode \
  /Users/jrudnik/.jcode/builds/shared-server/jcode
shasum -a 256 /Users/jrudnik/.jcode/builds/current/jcode \
  /Users/jrudnik/.jcode/builds/shared-server/jcode
```

The resulting hashes must match the two anchors above. The full rollback runbook
must then gracefully restart the intended menubar, hotkey, and shared-server
integrations using their supported lifecycle commands; no `kill -9` shortcut is
acceptable.

### Intended user-facing processes and agent

The following were live and must be classified **retain/preserve-and-restart**,
not swept as stale:

- `jcode menubar` from `/Users/jrudnik/.local/bin/jcode`;
- `jcode setup-hotkey --listen-macos-hotkey`;
- the shared-server process from `~/.jcode/builds/shared-server/jcode`;
- loaded LaunchAgent `com.jcode.hotkey` at
  `~/Library/LaunchAgents/com.jcode.hotkey.plist`.

A sandbox validation daemon must use disjoint home, runtime, socket, port, pid,
and marker paths. Its no-orphan requirement does not authorize silently killing
or replacing these pre-existing user integrations.

### Unrelated agent and ignored runtime state

Loaded LaunchAgent `com.jcode.lesson-library-shadow` runs a Python lesson-library
sampler under `docs/cloudflare-roadmap/`. It is **not** part of the jcode runtime,
is out of normalization scope, and must be retained unless the user separately
authorizes its management. Name or path matching alone never proves that a
process or LaunchAgent belongs to the jcode runtime.

The canonical checkout contains ignored `.wrangler/` state used by that unrelated
agent. Its presence does not appear in `git status` and is not evidence of jcode
runtime dirtiness. It remains out of scope.

Credential files and references under `~/.jcode` must be inventoried by metadata
only. Never print their contents into logs or committed evidence.

## Recovery evidence state

- Phase 6: complete.
- Coordinator audit: 62/62 real expected-exit checks matched.
- Evidence manifests: 17/17 verified.
- Independent reviews: Opus spot PASS, Fable architecture PASS, Sol final PASS,
  fresh Fable final PASS, with zero unresolved IMPORTANT or CRITICAL findings.
- W7 review: committed at
  `docs/fork/recovery/reviews/2026-07-16-w7-review.md`.

## Required session-start reproduction

Run this block at the start of **every** normalization session and append the
observation before resuming mutation. Inventory credential-bearing files by path,
type, ownership, mode, size, and timestamp only.

```bash
cd /Users/jrudnik/labs/jcode
git status --short
git branch --show-current
git rev-parse HEAD main recovery/2026-07-15 vendor/upstream
git log -1 --format='%H' -- docs/fork/normalization/COMPLETION_STANDARD.md
git rev-list --left-right --count main...recovery/2026-07-15
git for-each-ref refs/heads --format='%(refname) %(objectname)'
git for-each-ref refs/tags --format='%(refname) %(objectname)'
git rev-list --all --not recovery/2026-07-15 --count
git worktree list --porcelain
git stash list --format='%gd%x09%H%x09%gs'
for i in 0 1 2 3; do
  git rev-parse "stash@{$i}"
  git rev-parse "stash@{$i}^2" 2>/dev/null || true
done
git remote -v
git diff -- docs/fork/recovery/ORCHESTRATOR_PROMPT.md | shasum -a 256
type -a jcode
ls -ld ~/.local/bin/jcode ~/.jcode/builds/current/jcode \
  ~/.jcode/builds/shared-server/jcode \
  /etc/profiles/per-user/jrudnik/bin/jcode
ps -axo pid=,ppid=,command= | grep -E \
  '[j]code (menubar|setup-hotkey|daemon|serve)|[/]jcode([[:space:]]|$)' || true
launchctl print gui/$(id -u)/com.jcode.hotkey 2>/dev/null || true
launchctl print gui/$(id -u)/com.jcode.lesson-library-shadow 2>/dev/null || true
```

The next session must extend this into a complete read-only host inventory for
aliases, symlinks, application bundles, services, sockets, configuration,
caches, build queues, package/profile entries, and every filesystem path
containing jcode runtime or source state. This baseline does not claim those
surfaces are clean.

## Session observations (append-only)

### 2026-07-16T17:03Z session-start reproduction (N0 coordinator)

Reproduction block executed at HEAD `02e25ba331b0badab37580173dd943db457f4a36`
(`recovery/2026-07-15`). Matching facts: authority commit
`e94842b0045063e0a8a29a8985e551b8f02e70b3` reachable from HEAD; `main`
`6ca1fcf2e`; `vendor/upstream` `631935dd1`; 40 branches; 138 tags; 916 commits
reachable from local refs but not recovery HEAD; 29 worktrees; all four stash
commits and index parents byte-identical to the table above; remotes unchanged;
prompt diff SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`;
menubar (pid 30131), hotkey listener (pid 95512), shared server (pid 47209),
`com.jcode.hotkey` running, `com.jcode.lesson-library-shadow` loaded/not running;
Nix profile symlink unchanged.

Explained drift, recorded append-only, no mutation performed in response:

1. `main...recovery` is now `0 170` versus the snapshot's `0 166`. The four
   extra commits are the committed normalization infrastructure
   (`1f938b7e5`, `e94842b00`, `9264b37df`, `02e25ba33`). Expected evolution.
2. Both `~/.jcode/builds/current/jcode` (mtime Jul 16 11:16) and
   `~/.jcode/builds/shared-server/jcode` (mtime Jul 16 12:49) now point to
   `~/.jcode/builds/versions/02e25ba33-dirty-1706909ba396/jcode`, SHA-256
   `2e9438f311d886a8dc230acaec27287eb104a6b27d490dc6c02c50d4b95b6109`. This
   repoint happened through normal user selfdev build/reload activity after the
   snapshot, not through this normalization program. The two pre-promotion
   rollback anchor binaries remain present on disk and hash-match this baseline
   exactly (`fd6297d9…` at `versions/6c6a4f2c8-dirty-7b4ec829c656/jcode`,
   `a4973e8c…` at `versions/65cfde463/jcode`), so the recorded restore commands
   remain executable. The current `02e25ba33-dirty` targets and hash above are
   additionally recorded as the *session-start* live-runtime state; any
   normalization repoint must record and restore against whichever state is
   live at that moment, not only the original snapshot.
3. Untracked `opencode.json` (1680 bytes, mtime Jul 16 12:32) exists at the
   repository root. It is a user-created OpenCode provider configuration that
   references a credential by filesystem path only; no secret content is
   embedded. It is user configuration, not recovery or normalization output.
   Disposition (ignore, relocate, or commit) is a user decision; it is retained
   untouched and excluded from "unexplained dirty state."
