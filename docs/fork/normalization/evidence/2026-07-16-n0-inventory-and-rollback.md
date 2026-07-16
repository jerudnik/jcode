# N0 evidence: inventory, classification, and rollback archives

Recorded 2026-07-16T17:07Z at repository HEAD
`cffb1af2f1559298c1b969bb5b8e8dea4b5c693b` (`recovery/2026-07-15`). The
session-start reproduction and explained drift are appended to `BASELINE.md`
in the same history line.

## 1. Rollback archives (D0)

### Stash identity verification

All four stash worktree commits, all four index parents, and the `stash@{3}`
untracked payload parent `7c68ef5f59359ed89e0979b99bba143c74d926aa` were
verified byte-identical to the `BASELINE.md` table before archiving.

### Archive refs created (reversible, additive)

```
refs/archive/normalization/stashes/0/worktree  1f54abc9fbb0190f59af2fe5744e8e8dfb99c67f
refs/archive/normalization/stashes/0/index     f8bb6829da55c4754eb42c01f28d47c12f1c881c
refs/archive/normalization/stashes/1/worktree  975b91b8336122d55eb8d0955fb6aa09158e5b27
refs/archive/normalization/stashes/1/index     7385174012d5bf5dd8820d0cc7cd286e902cfa96
refs/archive/normalization/stashes/2/worktree  5dc53ed77b98effbd682402ddb10a6c6d6c286fe
refs/archive/normalization/stashes/2/index     608a17fbd08dad9c62d731b8d9edae65b5c1a4dc
refs/archive/normalization/stashes/3/worktree  29d49b250a6a7e924fa1beb33a07f635fc13c9be
refs/archive/normalization/stashes/3/index     78a8baeddb70ccdc989c1566e8bdf80ee582e3e3
refs/archive/normalization/stashes/3/untracked 7c68ef5f59359ed89e0979b99bba143c74d926aa
```

Expected reachability transition: `git rev-list --all --not
recovery/2026-07-15 --count` moved from `916` to `923` when the nine archive
refs made previously reflog-only stash objects ref-reachable. The seven newly
counted commits are exactly the 4 index parents plus stashes 1-3 worktree
commits minus stash 0 already counted via `refs/stash`, plus the untracked
payload parent; this is the transition `BASELINE.md` pre-declared as expected,
not drift.

### Bundles

Created in `/Users/jrudnik/labs/jcode-normalization-rollback/` (outside every
worktree; carried on the final cleanup manifest as N0-owned):

| Bundle | SHA-256 | Size |
|---|---|---|
| `jcode-stashes.bundle` | `f102fa69511ea31f261442b5e68e8621a02d2c88f884c34acbf47e502dcb7583` | 370331490 bytes |
| `jcode-all-refs.bundle` | `a29110621aa37b9ad142850305d6b72f5fd7ff9c84d9ba046a4eb4ec74254b82` | 473090497 bytes |

Both passed `git bundle verify` ("records a complete history").

### Restoration test (disposable repository, removed after test)

Both bundles were fetched into fresh bare repositories under
`/tmp/jcode-bundle-restore-QfADiS` (since deleted):

- all-ref restore contained 40 heads, 138 tags, 9 archive refs; all 187
  original branch/tag/archive tips verified present object-by-object;
- all nine stash objects, including untracked parent
  `7c68ef5f59359ed89e0979b99bba143c74d926aa`, verified present in **both**
  restores via `git cat-file -e <sha>^{commit}`;
- `git fsck` (stash restore) and `git fsck --connectivity-only` (all-ref
  restore) reported no errors.

D0 bundle gates therefore pass. Note the all-ref bundle was created *after*
the archive refs, so it also physically carries the stash objects; the
separate stash bundle is retained as the required independent guarantee.

## 2. Ref-by-ref classification (provisional dispositions)

All deletions remain approval-gated; classification here is inventory, not
action. `archive` means preserve under `refs/archive/` before any deletion of
the working ref.

### Branches (40)

| Branch | Classification | Rationale |
|---|---|---|
| `main` | canonical (promotion target) | moves only by reviewed N2 fast-forward |
| `recovery/2026-07-15` | archive (immutable) | signed recovery line; N1 creates `refs/archive/recovery/2026-07-15` |
| `vendor/upstream` | retain (policy refresh in N3) | pinned upstream mirror; stale-footgun handling in N3 |
| `sync/upstream-v0.46` | user decision | pre-recovery sync lane; not ancestor of recovery |
| `follow-upstream` | user decision | old tracking lane |
| `distro/nix`, `feat/nix-managed-mode` | user decision | Nix packaging lanes, possibly active user work |
| `backup/pre-stabilization-2026-07-14` | archive | explicit backup ref |
| `agent/hotpath-stabilization`, `agent/marker-hardening` | archive | superseded agent lanes; content preserved in bundles |
| `fix/mcp-selfspawn-supervision-hardening` | user decision | may contain unintegrated fix |
| `orch/*` (4) | archive | orchestrator experiment lanes |
| `recovery/fix-*`, `recovery/seam-*`, `recovery/light-*`, `recovery/orchestrator-s{4,5,6}-*`, `recovery/pilot-prereq-*`, `recovery/docs-fork-governance-*`, `recovery/close-*` (23) | archive then remove | recovery working lanes fully represented in the recovery line and bundles; per-branch ancestry check required in N1 reconciliation before removal |

### Tags (138)

`v*` release tags (132): retain. `presync/*` (3): archive; recovery-era safety
anchors. `readme-assets`, `tb21-confstep-943eae93`, `tui-refactor-base-20260306`:
retain pending user decision; low cost.

### Stashes (4)

All four are `On main` WIP from pre-recovery work, now object-archived. N1
reconciliation must determine per-stash whether the hot-path work was already
integrated (PROGRESS.md records "already-landed fixes and preserved hot-path
stashes are evidence, not replay instructions"), then propose keep/drop in an
approval packet. No drop is authorized yet.

### Worktrees (29)

Primary `/Users/jrudnik/labs/jcode`: canonical. All 28 auxiliary worktrees
re-verified clean this session (dirty=0 each; 0 prunable). Provisional: all 24
recovery-lane labs worktrees plus the 4 `/private/tmp` worktrees classify
`remove after ref archive + approval`. `jcode-governance-decisions` follows the
same rule once its branch is confirmed merged.

## 3. Host inventory summary and classification

### Binaries and PATH

| Item | Classification |
|---|---|
| `/Users/jrudnik/.local/bin/jcode -> ~/.jcode/builds/current/jcode` | canonical user-selected runtime link |
| `/Users/jrudnik/.local/bin/jcode.selfdev-backup` (same target, Jul 8) | remove candidate (redundant duplicate symlink) |
| `/etc/profiles/per-user/jrudnik/bin/jcode` (Nix/home-manager) | retain; declaratively managed, never mutated directly |
| `~/.jcode/builds/current/jcode -> versions/02e25ba33-dirty-1706909ba396/jcode` | live runtime; session-start hash `2e9438f311d886a8dc230acaec27287eb104a6b27d490dc6c02c50d4b95b6109` |
| `~/.jcode/builds/shared-server/jcode -> versions/02e25ba33-dirty-1706909ba396/jcode` | live shared-server runtime (same target/hash) |
| `~/.jcode/builds/versions/` (7 entries incl. both BASELINE rollback anchors) | retain until post-promotion cleanup approval; anchors `6c6a4f2c8-dirty-*` and `65cfde463` verified present and hash-matching |
| `~/Applications/Jcode.app` | user decision (desktop app bundle) |

### Processes and services

| Item | Classification |
|---|---|
| `jcode menubar` (pid 30131) | retain / graceful restart only |
| `jcode setup-hotkey --listen-macos-hotkey` (pid 95512) | retain / graceful restart only |
| shared server `castle` (pid 47209, sockets `$TMPDIR/jcode.sock`, `$TMPDIR/jcode-debug.sock`, git 02e25ba33) | retain / graceful restart only |
| LaunchAgent `com.jcode.hotkey` (running) | retain |
| LaunchAgent `com.jcode.lesson-library-shadow` (loaded, not running) | unrelated, out of scope |
| `.wrangler/` ignored state in checkout | unrelated, out of scope |

### `~/.jcode` profile

Credential files (`auth.json`, `openai-auth.json`, `antigravity_oauth.json`,
plus `.bak` and refresh/validation state) inventoried metadata-only: mode 600
(refresh/validation 644), owner jrudnik, sizes 478-4376 bytes. Classification:
retain, never printed. `config.nix.toml` and `mcp.json` are home-manager
symlinks into `/nix/store`: retain, declarative. `config.toml`,
sessions (246M), logs (1.8G), memory, goals, todos, skills: retain (user
data). `recovery-backup-20260716-1245{,56}`: retain until N5 approval packet.
`selfdev-build-requests/` (11 completed request records, empty lock dir):
remove candidate at N5. `active_pids`/`streaming_pids` single live session
entry: live state, retain.

### Temporary filesystem

`/private/tmp` holds ~977 `jcode*` entries: 4 registered worktrees (classified
above), recovery evidence/review scratch files, and ~900+ timestamped
disposable test homes (`jcode-burst-spawn-*`, `jcode-reload-*`, etc.).
Classification: recovery evidence scratch already superseded by committed
evidence -> remove candidates; test-home directories -> remove candidates;
all subject to the N5 dry-run manifest and approval. Nothing under
`/private/tmp` is deleted by N0.

### Repository-root untracked file

`opencode.json`: user OpenCode provider config (credential referenced by path
only, no embedded secret). Classification: user decision; retained untouched.

## 4. Approval-gated action register

The following categories require explicit approval packets before execution:

1. moving `main` (N2 exit);
2. deleting any branch, tag, or archive ref;
3. dropping any stash;
4. removing any worktree;
5. repointing `~/.jcode/builds/current` or `shared-server` links;
6. stopping/restarting menubar, hotkey, shared-server, `com.jcode.hotkey`;
7. any change to Nix/home-manager-managed paths (via declarative source only);
8. deleting `/private/tmp` scratch, `~/.jcode` backups, build queue records,
   or the rollback bundle directory;
9. real provider credentials, installer/updater execution, publication, or
   any remote push;
10. disposition of the preserved `ORCHESTRATOR_PROMPT.md` edit and
    `opencode.json`.

Inventory and deletion were not combined in any N0 step. No destructive action
was taken in N0.
