# Fork rewrite-file seam audit

Change 3 of the fork-sustainability model (`FORK_SUSTAINABILITY_MODEL.md`). This
walks every source file where the fork *deletes or rewrites* upstream lines (the
only real conflict source) and judges, per file, whether the edit can become an
**additive seam** (new file / new function / trait default + one call site) so
the 6h rebase stops conflicting there.

Method: `git diff $(git merge-base HEAD github/vendor/upstream)..HEAD`, counting
deleted upstream `.rs` lines per file. Threshold for "rewrite-file": >5 deleted
non-test upstream lines.

## The seven rewrite-files (added / deleted)

| File | +add | -del | Nature | Seam verdict |
|------|-----:|-----:|--------|--------------|
| `provider-anthropic/src/lib.rs` | 269 | 58 | extract upstream inline tool-list into a named fn + add `oauth_extra_tools` | **Mostly additive already**; the 58 deletions are an extract-refactor. Low residual risk. |
| `base/src/skill.rs` | 31 | 20 -> **0** | replace two `.jcode`/`.claude` if-blocks with a 4-dir loop | **DONE -> additive**: upstream's blocks restored byte-identical; `.apm`/`.agents` now load via a prepended loop. Zero upstream deletions. |
| `src/cli/acp.rs` | 626 | 19 | large ACP feature build-out + small upstream call-site edits | **Leave**: 33:1 additive; the 19 deletions are unavoidable call-site rewrites. |
| `base/src/mcp/protocol.rs` | 128 | 18 | custom `Deserialize` impl + `.apm`/config-merge | **Partly additive**: the custom deserializer is real; the import churn is forced by it. Low risk. |
| `terminal-launch/src/lib.rs` | 74 | 10 | add `TERM_PROGRAM=ghostty` detection + `-e` arg | **Leave**: edits an upstream `if` and match; small, genuine behavior change. |
| `base/src/config.rs` | (many) | 8 | insert `Assistant*` types into a sorted `use` list + new field | **Unavoidable**: see below. rerere territory. |
| `tui/src/tui/mod.rs` | (small) | 7 | `assistant_status` trait default + 2 churn deletions | **Partly fixable**: one real seam, two avoidable churn lines. See below. |

## Two findings that change the framing

### 1. Some "rewrites" are upstream shipping unformatted code, not our churn

`tui/mod.rs`'s `has_started_conversation` shows as a 5-line delete + 4-line add,
but the logic is identical. Reason: upstream committed a multi-line `matches!`
that **`cargo fmt` collapses to one line** (88 cols, under the 100 default). Our
tree is the *formatted* form. Verified:

```
$ rustfmt --edition 2024 <upstream form>   # collapses to our single-line form
```

So reverting to upstream's form is pointless (the next `cargo fmt` re-expands
it), and the conflict is upstream's unformatted commit, not a downstream edit.
The only durable fixes are external to us: upstream runs `cargo fmt`, or we let
rerere absorb it. Do **not** "fix" this by hand.

The `session_picker`/`session_facts` reorder in the same file is the same story:
an alphabetization rustfmt/import-ordering difference, not a semantic edit.

### 2. Adding a type to a sorted `use` list necessarily reflows it

`config.rs`'s 8 deletions are entirely the re-wrapping of an alphabetically
sorted, rustfmt-wrapped `use` block when `AssistantMemoryScope`, `AssistantMode`,
`AssistantProfile`, `AssistantProfileError`, `AssistantProfilesConfig` are
inserted. There is no additive seam for "I imported five new names that sort into
the middle of an existing wrapped list" -- the line wrapping moves. This is
exactly the recurring, mechanical conflict `git rerere` was shipped (Change 1) to
record once and replay. Accept it; do not contort the imports to dodge it.

## Concrete actions, ranked

1. **`skill.rs` -> additive loop (do).** Keep upstream's `.jcode`/`.claude`
   blocks verbatim and add downstream discovery as a separate appended step:

   ```rust
   // upstream block unchanged ...
   // downstream addition (additive seam):
   for dir_name in [".apm", ".agents"] {
       let dir = Self::project_local_dir(working_dir, dir_name);
       if dir.exists() { self.load_from_dir(&dir)?; }
   }
   ```

   Trade-off: ordering. Upstream loads `.jcode` then `.claude`; if `.apm`/
   `.agents` must win precedence the appended-loop order differs. If precedence
   does not matter, this removes the conflict entirely. **Worth doing** because
   `skill.rs` is small and the rewrite is gratuitous given an append works.

2. **`tui/mod.rs` -> stop carrying the two churn lines (do, if upstream cannot
   format).** The `assistant_status` trait default is already the ideal additive
   seam; keep it. The import reorder and the `has_started_conversation` reflow
   are pure-format deltas -- ideally fixed by upstreaming a `cargo fmt` commit;
   otherwise leave them to rerere. No hand-edit.

3. **`provider-anthropic/lib.rs` -> upstream the extraction (optional).** The
   biggest delete count is an extract-method refactor (inline list -> named fn).
   If upstream took that same refactor (a pure no-behavior-change extraction),
   our 58-line delete collapses to ~0. A clean upstream PR candidate.

4. **`acp.rs`, `terminal-launch`, `mcp/protocol.rs`, `config.rs` -> leave.**
   Their deletions are either tiny unavoidable call-site edits (acp,
   terminal-launch), forced by a legitimate new impl (mcp), or forced by sorted
   imports (config). All four are dominated by additions and are squarely in
   "let rerere replay it" territory.

## Standing rule (unchanged, reinforced)

New features add a file + one registration line. The audit shows the *existing*
debt is small and mostly unavoidable; the one gratuitous rewrite (`skill.rs`) is
the only hand-fix worth making. Everything else is either upstream's formatting
or the irreducible cost of adding real types -- which is what Change 1 (rerere)
exists to absorb. No heavyweight tooling is justified by these seven files.
