# Invalid unsafe/preaccepted validation attempts

This directory preserves W5 validation artifacts produced before the accepted no-Nix evidence protocol was finalized.

Invalidation reasons:

- Some earlier driver revisions used `scripts/dev_cargo.sh` and/or plain `nix develop`, which could permit substituters or remote builders despite Cargo offline mode.
- Later hardening required accepted W5 evidence to use no Nix invocation at all, only cached `/nix/store/.../rust-default-1.96.0/bin/{cargo,rustfmt}` and `/Library/Developer/CommandLineTools/usr/bin/python3`.
- A direct-tool run was started before the top-level evidence directory was cleaned and was cancelled. Its partial outputs are preserved here with the prior unsafe/preaccepted logs so the accepted top-level result can be produced by exactly one clean driver execution.
- One preservation command observed a missing top-level `SHA256SUMS` after earlier partial cleanup; the remaining files were moved idempotently in the follow-up command.

These files are preserved for audit only and are not accepted validation evidence.

## Compression correction

After preservation, the raw `.log` files and `cargo_path.stderr` were deterministically gzip-compressed with empty gzip filename and mtime `0` so the final base-relative `git diff --check` is green while byte-exact decompression remains available. `RAW_SHA256SUMS` records the SHA-256 of each uncompressed raw file, and `SHA256SUMS` records the tracked compressed files and incident metadata.
