# R07 barrier 0 — coordinator authorization evidence

Date: 2026-07-28. Per design §11 barrier 0: "coordinator resolves workflow
ownership, confirms external GitHub writes, and proves the archive repository
is private."

## 1. Archive repository privacy proof (read-only)

Command (ambient auth, GET only):

```text
gh api repos/jerudnik/jcode-recovery-archive --jq '{full_name, private, visibility, fork, default_branch}'
```

Result:

```json
{"default_branch":"main","fork":false,"full_name":"jerudnik/jcode-recovery-archive","private":true,"visibility":"private"}
```

`jerudnik/jcode-recovery-archive` is private, not a fork, default branch
`main`. Matches Stream A's read-only identity confirmation (HEAD + 44 refs,
zero tags, zero managed archive refs in `stream-a-pre-write-manifest.md`).

## 2. Workflow ownership resolution

`workflow-contexts.proposed.patch` (handed off by Stream G unapplied) is owned
by the coordinator. It will be applied to `.github/workflows/` only as part of
the authorized bootstrap integration PR (barrier 2), never pushed unverified:
the patch was `git apply --check`-clean against current main, applied to a
scratch tree, and all resulting workflows passed actionlint (integration
adjudication record).

## 3. External-write authorization

The remaining external writes are enumerated for user authorization:

1. **Archive push (barrier 1):** the single atomic command in
   `stream-a-refspecs.md` — 33 reviewed heads + 6 stash tags to
   `https://github.com/jerudnik/jcode-recovery-archive.git`, no force, no
   deletions, no fallback if `--atomic` is rejected. Followed by a
   fresh-fetch reachability verification of all 39 refs.
2. **Push of R07 branches to jerudnik/jcode** and the **bootstrap integration
   PR** (barrier 2): R07 implementation + authorized workflow diff +
   coordinator-applied STATE schema-v2 migration. Created via
   `gh pr create -R jerudnik/jcode`, merge-commit only.
3. **Context-emission proof (barrier 3):** read-only observation on the
   bootstrap PR (all four contexts emitted by integration id 15368;
   `Governance Root` expected red naming the governance paths changed).
4. **Bootstrap merge (barrier 4):** API merge with expected head SHA,
   `merge_method=merge`, then the `github-governance.proposed.json` apply
   sequence (ruleset PUTs, repo merge-settings PATCH, classic-protection
   DELETE last, each write followed by its named read-back).
5. **Under-enforcement proof PR (barrier 5):** a harmless PR observing all
   four contexts green, plus a planted workflow change observing
   `Governance Root` red, closed unmerged.
6. **Final evidence + R07 checkpoint (barriers 6-7):** sanitized transcripts,
   live fork-health rerun, and the coordinator checkpoint of R07 itself.

All writes use the admin token read inline only via
`rbw get jcode-temp-admin-key`, never stored or echoed. No pushes to any
repository other than jerudnik/jcode and jerudnik/jcode-recovery-archive.

Status: items 1-2 proven/resolved by the coordinator; item 3 awaits explicit
user authorization before any external write executes.
