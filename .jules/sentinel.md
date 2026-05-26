## 2025-02-28 - [CRITICAL] Fix File Creation Race Condition for Secrets
**Vulnerability:** File Creation Race Condition in `crates/jcode-storage/src/lib.rs`
**Learning:** `write_text_secret` and `write_json_secret` were creating temporary files before tightening permissions on those files, which exposed the secret material for a brief amount of time to the operating system's default umask permissions (e.g. world-readable).
**Prevention:** I modified the `write_bytes_inner` helper that both operations share to accept a `secret: bool` parameter. Then, on `cfg(unix)` environments, instead of creating a file directly using `std::fs::File::create`, I used `std::fs::OpenOptions` and set `std::os::unix::fs::OpenOptionsExt::mode(0o600)` to force the file permissions *upon creation*. This removes the time window in which the file would otherwise exist on disk with default permissions.
## 2026-05-26 - [Secret Input Echo Prevention]
**Vulnerability:** Google OAuth Client Secret was being echoed to the terminal during CLI login.
**Learning:** Hardcoded `io::stdin().read_line()` was used for secret input, which does not hide typing. Found an existing `read_secret_line()` utility in `src/cli/login.rs` that handles terminal raw mode.
**Prevention:** Use `read_secret_line()` for any sensitive inputs in the CLI to prevent secret leakage via terminal history or shoulder surfing.
