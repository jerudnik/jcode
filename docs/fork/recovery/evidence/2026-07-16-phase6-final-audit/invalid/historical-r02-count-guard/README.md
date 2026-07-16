# Invalid Phase 6 attempt: historical R02 count guard

This attempt ran at `51168d16e9c708ae4afff09a6fc6402642d17782` and stopped after the R02 subscription suite itself passed `38/38`, because the driver still required the historical combined-prerequisite count `35`. The count-guard command exited `1`; no product test failed. The attempt is invalid final evidence and is preserved append-only.

The original driver file was corrected in place before the accepted rerun, so it is not represented as byte-preserved here. `manifest.tsv`, every produced raw log, and their decompressed SHA-256 values are byte-preserved. The only accepted driver correction changed the required R02 count from `35` to `38`.
