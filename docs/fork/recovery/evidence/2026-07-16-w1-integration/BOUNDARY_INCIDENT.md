# W1 post-integration validation boundary incident

The first post-integration focused-test attempt used
`bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- r12_ --nocapture`.
Because ambient `cargo` was absent, `dev_cargo.sh` re-entered the repository Nix
development shell without an explicit `--offline` argument. The shell hook found
its fork-nudge timestamp stale and launched the documented background command
`git fetch --quiet --prune github`. This was not authorized by the recovery
workflow's offline boundary. The attempt remains preserved in
`attempt-history/initial-dev-cargo-r12.log.gz` and is not the authoritative
post-integration validation result.

The fetch completed at 2026-07-16T03:47:39Z (2026-07-15T23:47:39-0400). It
updated `.git/FETCH_HEAD` and `.git/fork-nudge-last-fetch`, but moved no
remote-tracking ref: the newest `github/*` reflog entry remained
`github/main@{2026-07-14T14:31:45-04:00}`, and the fetched hashes exactly matched
the existing `github/distro/nix`, `github/follow-upstream`, `github/main`, and
`github/vendor/upstream` refs. No fetch/network process remained when checked.
Local branches, W1 and W2 worktree HEADs, the sole dirty prompt diff, and all
four stashes were unchanged.

The authoritative replacement command set
`FORK_NUDGE_MAX_AGE=2147483647`, `FORK_NUDGE_AUTOSYNC=0`, and
`JCODE_NO_TELEMETRY=1`, then ran `nix develop --offline` with a disposable
`JCODE_HOME`. Its transcript contains no remote-refresh message, no fetch
process remained afterward, and all 11 focused R12 fixtures passed.

Two additional non-authoritative attempts remain visible:

- The first static R09 wrapper included an erroneous auxiliary `bash -n`
  invocation for a nonexistent/mixed file list and reported exit `127`; the
  authoritative `bash -n scripts/*.sh` check in the same run exited `0`, and the
  exact matrix was rerun cleanly afterward.
- Workspace `cargo fmt -- --check` reproduced pre-existing formatting drift in
  files outside W1. No file was changed. Targeted `rustfmt --check` over the four
  W1 Rust files subsequently exited `0`.

No provider, credential, daemon, reload, tool/MCP, publication, installation,
updater, release, or quality-baseline action occurred. No `--update` was used.

All six command transcripts are stored as deterministic `gzip -n` archives.
`gzip -dc <file>.gz` reproduces the original transcript bytes.
