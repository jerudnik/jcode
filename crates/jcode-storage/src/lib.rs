use anyhow::Result;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

mod active_pids;
pub use active_pids::{
    PidMarkerSweep, SessionCounts, SessionPidMarkerObservations, SessionPidMarkerRemoval,
    SessionPresence, StreamingGuard, active_pids_dir, active_session_ids,
    find_active_session_id_by_pid, mark_streaming, observe_session_pid_markers,
    register_active_pid, remove_active_pid_marker_if_stale_and_matches,
    remove_session_pid_markers_if_unchanged, session_counts, session_presence, streaming_pids_dir,
    sweep_stale_pid_markers, unmark_streaming, unregister_active_pid,
};

/// Platform-aware runtime directory for sockets and ephemeral state.
///
/// - Linux: `$XDG_RUNTIME_DIR` (typically `/run/user/<uid>`)
/// - macOS: `$TMPDIR` (per-user, e.g. `/var/folders/xx/.../T/`)
/// - Fallback: `std::env::temp_dir()`
///
/// Can be overridden with `$JCODE_RUNTIME_DIR`.
pub fn runtime_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("JCODE_RUNTIME_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(dir) = std::env::var("TMPDIR") {
            return PathBuf::from(dir);
        }
    }

    let dir = fallback_runtime_dir();
    ensure_private_runtime_dir(&dir);
    dir
}

fn fallback_runtime_dir() -> PathBuf {
    std::env::temp_dir().join(format!("jcode-{}", runtime_user_discriminator()))
}

#[cfg(unix)]
fn runtime_user_discriminator() -> String {
    unsafe { libc::geteuid() }.to_string()
}

#[cfg(not(unix))]
fn runtime_user_discriminator() -> String {
    let raw = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "user".to_string());
    let sanitized: String = raw
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .take(64)
        .collect();
    if sanitized.is_empty() {
        "user".to_string()
    } else {
        sanitized
    }
}

fn ensure_private_runtime_dir(path: &Path) {
    let _ = std::fs::create_dir_all(path);
    #[cfg(unix)]
    {
        let _ = jcode_core::fs::set_directory_permissions_owner_only(path);
    }
}

pub fn jcode_dir() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("JCODE_HOME") {
        return Ok(PathBuf::from(path));
    }

    if let Some(dir) = test_harness_home() {
        return Ok(dir);
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
    Ok(home.join(".jcode"))
}

/// Per-process fallback home for test binaries that never set `JCODE_HOME`.
///
/// Without this, any test that reaches [`jcode_dir`] transitively reads and
/// writes the developer's real `~/.jcode`. That is not a tidiness problem, it
/// is a correctness one, and it has bitten this repo repeatedly: a test read a
/// real provider credential, another raced on the real config cache, another
/// loaded the real ambient queue. It also leaks: a single `--workspace` run
/// deposited thousands of stub sessions into `~/.jcode/sessions/`.
///
/// Redirecting rather than failing keeps the blast radius at zero. A test that
/// wants the real home says so with `JCODE_ALLOW_REAL_HOME_IN_TESTS=1`; a test
/// that wants a specific home still sets `JCODE_HOME` and wins outright. Note
/// this is process-scoped, not test-scoped: tests within one binary still share
/// a home, exactly as they shared the real one before. It removes cross-machine
/// and cross-binary coupling, not the need for per-test isolation.
fn test_harness_home() -> Option<PathBuf> {
    if !running_under_test_harness() {
        return None;
    }

    static HOME: OnceLock<PathBuf> = OnceLock::new();
    Some(
        HOME.get_or_init(|| {
            let dir = std::env::temp_dir().join("jcode-test-homes").join(format!(
                "{}-{}",
                std::process::id(),
                rand::random::<u32>()
            ));
            // Reuses the runtime-dir helper: same best-effort create + owner-only
            // permissions. The path is returned whether or not creation
            // succeeded, because falling back to the real home on failure would
            // silently reintroduce exactly the leak this exists to prevent.
            // Callers create the subdirectories they write into, so a genuine
            // failure surfaces there with its own context.
            ensure_private_runtime_dir(&dir);
            dir
        })
        .clone(),
    )
}

/// Whether this process is a `cargo test` / `cargo bench` harness binary.
///
/// `cfg!(test)` cannot answer this: it is false in *this* crate whenever
/// another crate's test binary calls in, which is precisely the leaking case.
/// The reliable cross-crate signal is the layout cargo gives harness binaries,
/// `target/<profile>/deps/<name>-<hash>`, which no shipped binary has.
/// Verified empirically: `cargo run` and direct execution both resolve to
/// `target/<profile>/<name>`, outside `deps/`, while a test binary resolves to
/// `target/debug/deps/probe_t-a39295051af45621`.
///
/// Integration tests under `tests/` get the same layout, so they are covered.
/// A test that spawns a real jcode binary is unaffected: the child is not a
/// harness binary and resolves the real home, which is what such a test means.
fn running_under_test_harness() -> bool {
    if std::env::var_os("JCODE_ALLOW_REAL_HOME_IN_TESTS").is_some() {
        return false;
    }
    match std::env::current_exe() {
        Ok(exe) => is_cargo_test_binary_path(&exe),
        Err(_) => false,
    }
}

/// Pure classifier behind [`running_under_test_harness`], split out so the
/// layout rule can be tested without spawning processes.
fn is_cargo_test_binary_path(exe: &Path) -> bool {
    // Require both the `deps` parent and cargo's metadata hash in the stem, so
    // a binary that merely lives in some directory named `deps` is not
    // misclassified as a test.
    if exe.parent().and_then(|p| p.file_name()) != Some(std::ffi::OsStr::new("deps")) {
        return false;
    }
    let Some(stem) = exe.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    match stem.rsplit_once('-') {
        Some((name, hash)) => {
            !name.is_empty() && hash.len() >= 16 && hash.chars().all(|c| c.is_ascii_hexdigit())
        }
        None => false,
    }
}

pub fn logs_dir() -> Result<PathBuf> {
    Ok(jcode_dir()?.join("logs"))
}

/// Durable state directory for state that must survive reboots.
///
/// [`runtime_dir`] typically resolves to a tmpfs (for example
/// `/run/user/<uid>` on Linux) that is wiped on reboot, so it must only hold
/// sockets and truly ephemeral state. State that has to outlive a reboot,
/// such as swarm plans and member records, belongs here instead: it resolves
/// to `~/.jcode/state` (respecting `JCODE_HOME`).
///
/// When `JCODE_RUNTIME_DIR` is set (tests and sandboxed temp servers), it
/// takes precedence so isolated runs never touch the real jcode home.
pub fn durable_state_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("JCODE_RUNTIME_DIR") {
        return PathBuf::from(dir).join("durable-state");
    }
    match jcode_dir() {
        Ok(dir) => dir.join("state"),
        Err(_) => runtime_dir().join("durable-state"),
    }
}

/// Resolve jcode's app-owned config directory.
///
/// Default location is the platform config dir + `jcode` (for example
/// `~/.config/jcode` on Linux). When `JCODE_HOME` is set, sandbox this under
/// `$JCODE_HOME/config/jcode` so self-dev/tests do not leak into the user's
/// real config directory.
pub fn app_config_dir() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("JCODE_HOME") {
        return Ok(PathBuf::from(path).join("config").join("jcode"));
    }

    let config_dir =
        dirs::config_dir().ok_or_else(|| anyhow::anyhow!("No config directory found"))?;
    Ok(config_dir.join("jcode"))
}

/// Resolve a path under the user's home directory, but sandbox it under
/// `$JCODE_HOME/external/` when `JCODE_HOME` is set.
///
/// This keeps external provider auth files isolated during tests and sandboxed
/// runs without changing default on-disk locations for normal users.
pub fn user_home_path(relative: impl AsRef<Path>) -> Result<PathBuf> {
    let relative = relative.as_ref();
    if relative.is_absolute() {
        anyhow::bail!(
            "user_home_path expects a relative path, got {}",
            relative.display()
        );
    }

    if let Ok(path) = std::env::var("JCODE_HOME") {
        return Ok(PathBuf::from(path).join("external").join(relative));
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
    Ok(home.join(relative))
}

/// Best-effort startup hardening for local config dirs that may store credentials.
///
/// This intentionally ignores failures so startup does not fail on exotic
/// filesystems, but it narrows exposure on typical Unix systems.
pub fn harden_user_config_permissions() {
    if let Some(config_dir) = dirs::config_dir() {
        let jcode_config_dir = config_dir.join("jcode");
        if jcode_config_dir.exists() {
            let _ = jcode_core::fs::set_directory_permissions_owner_only(&jcode_config_dir);
        }
    }

    if let Ok(jcode_home) = jcode_dir()
        && jcode_home.exists()
    {
        let _ = jcode_core::fs::set_directory_permissions_owner_only(&jcode_home);
    }
}

/// Best-effort hardening for a secret-bearing file and its parent directory.
///
/// This is used before reading credential files so legacy permissive modes can
/// be tightened opportunistically.
pub fn harden_secret_file_permissions(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = jcode_core::fs::set_directory_permissions_owner_only(parent);
    }
    if path.exists() {
        let _ = jcode_core::fs::set_permissions_owner_only(path);
    }
}

/// Validate an external auth file managed by another tool before reading it.
///
/// jcode intentionally avoids mutating these files. We also reject obvious risky
/// cases like symlinks so a remembered trust decision stays bound to a real file
/// path rather than an arbitrary redirect.
pub fn validate_external_auth_file(path: &Path) -> Result<PathBuf> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to inspect external auth file {}: {}",
            path.display(),
            e
        )
    })?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!(
            "Refusing to read external auth file via symlink: {}",
            path.display()
        );
    }
    if !metadata.is_file() {
        anyhow::bail!(
            "External auth path is not a regular file: {}",
            path.display()
        );
    }
    std::fs::canonicalize(path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to canonicalize external auth file {}: {}",
            path.display(),
            e
        )
    })
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
        jcode_core::fs::set_directory_permissions_owner_only(path)?;
    }
    Ok(())
}

pub fn write_text_secret(path: &Path, content: &str) -> Result<()> {
    write_bytes_inner(path, content.as_bytes(), true)?;
    if let Some(parent) = path.parent() {
        jcode_core::fs::set_directory_permissions_owner_only(parent)?;
    }
    jcode_core::fs::set_permissions_owner_only(path)?;
    Ok(())
}

pub fn upsert_env_file_value(path: &Path, env_key: &str, value: Option<&str>) -> Result<()> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let prefix = format!("{}=", env_key);

    let mut lines = Vec::new();
    let mut replaced = false;
    for line in existing.lines() {
        if line.starts_with(&prefix) {
            replaced = true;
            if let Some(value) = value {
                lines.push(format!("{}={}", env_key, value));
            }
        } else {
            lines.push(line.to_string());
        }
    }

    if !replaced && let Some(value) = value {
        lines.push(format!("{}={}", env_key, value));
    }

    let mut content = lines.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    write_text_secret(path, &content)
}

pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    write_json_inner(path, value, true)
}

pub fn write_json_secret<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    write_json_inner(path, value, true)?;
    if let Some(parent) = path.parent() {
        jcode_core::fs::set_directory_permissions_owner_only(parent)?;
    }
    jcode_core::fs::set_permissions_owner_only(path)?;
    Ok(())
}

/// Fast JSON write: atomic rename but no fsync. Good for frequent saves where
/// durability on power loss is not critical (e.g., session saves during tool execution).
/// Data is still safe against process crashes (atomic rename protects against partial writes).
pub fn write_json_fast<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    write_json_inner(path, value, false)
}

/// Atomically write raw bytes to `path` (temp file + rename), fsync'd for
/// durability. Used for editing user config files where a torn write would be
/// catastrophic.
pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    write_bytes_inner(path, bytes, true)
}

fn write_json_inner<T: Serialize + ?Sized>(path: &Path, value: &T, durable: bool) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    write_bytes_inner(path, &bytes, durable)
}

fn write_bytes_inner(path: &Path, bytes: &[u8], durable: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }

    let pid = std::process::id();
    let nonce: u64 = rand::random();
    let tmp_path = path.with_extension(format!("tmp.{}.{}", pid, nonce));

    let result = (|| -> Result<()> {
        let file = std::fs::File::create(&tmp_path)?;
        let mut writer = std::io::BufWriter::new(file);
        writer.write_all(bytes)?;
        let file = writer
            .into_inner()
            .map_err(|e| anyhow::anyhow!("flush failed: {}", e))?;

        if durable {
            file.sync_all()?;
        }

        if path.exists() {
            let bak_path = path.with_extension("bak");
            // Preserve the previous version as .bak without ever leaving the
            // primary path missing. On Unix, rename(tmp, path) atomically
            // replaces the destination, so the backup can be a hard link to
            // the old inode: concurrent readers always see either the old or
            // the new content, never ENOENT. (The old rename-away approach
            // opened a window where the primary did not exist, which made
            // concurrent load-all style readers silently drop entries, e.g.
            // self-dev build requests "disappearing" from the queue.)
            #[cfg(unix)]
            {
                let _ = std::fs::remove_file(&bak_path);
                let _ = std::fs::hard_link(path, &bak_path);
            }
            // On Windows, rename fails when the destination exists, so the
            // primary must be moved away first; the brief missing window is
            // unavoidable without platform-specific replace APIs.
            #[cfg(not(unix))]
            {
                let _ = std::fs::rename(path, &bak_path);
            }
        }

        std::fs::rename(&tmp_path, path)?;

        #[cfg(unix)]
        if durable
            && let Some(parent) = path.parent()
            && let Ok(dir) = std::fs::File::open(parent)
        {
            let _ = dir.sync_all();
        }

        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }

    result
}

pub enum StorageRecoveryEvent<'a> {
    CorruptPrimary {
        path: &'a Path,
        error: &'a serde_json::Error,
    },
    RecoveredFromBackup {
        backup_path: &'a Path,
    },
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    read_json_with_recovery_handler(path, |event| match event {
        StorageRecoveryEvent::CorruptPrimary { path, error } => {
            eprintln!(
                "Corrupt JSON at {}, trying backup: {}",
                path.display(),
                error
            );
        }
        StorageRecoveryEvent::RecoveredFromBackup { backup_path } => {
            eprintln!("Recovered from backup: {}", backup_path.display());
        }
    })
}

pub fn read_json_with_recovery_handler<T, F>(path: &Path, mut on_recovery: F) -> Result<T>
where
    T: DeserializeOwned,
    F: FnMut(StorageRecoveryEvent<'_>),
{
    let data = std::fs::read_to_string(path)?;
    match serde_json::from_str(&data) {
        Ok(val) => Ok(val),
        Err(e) => {
            let bak_path = path.with_extension("bak");
            if bak_path.exists() {
                on_recovery(StorageRecoveryEvent::CorruptPrimary { path, error: &e });
                let bak_data = std::fs::read_to_string(&bak_path)?;
                match serde_json::from_str(&bak_data) {
                    Ok(val) => {
                        on_recovery(StorageRecoveryEvent::RecoveredFromBackup {
                            backup_path: &bak_path,
                        });
                        let _ = std::fs::copy(&bak_path, path);
                        Ok(val)
                    }
                    Err(bak_err) => Err(anyhow::anyhow!(
                        "Corrupt JSON at {} ({}), backup also corrupt ({})",
                        path.display(),
                        e,
                        bak_err
                    )),
                }
            } else {
                Err(anyhow::anyhow!("Corrupt JSON at {}: {}", path.display(), e))
            }
        }
    }
}

/// Fast append of a single JSON value followed by a newline.
/// Intended for append-only journals where per-write fsync is not required.
///
/// The entire line (value + trailing newline) is serialized into one buffer
/// and appended with a single `write_all`. Streaming the serializer straight
/// into the file issued many small writes, so a concurrent reader (or a
/// process killed mid-append) could observe a torn half-line, and two
/// concurrent appenders could interleave fragments. A single `O_APPEND` write
/// of the complete line keeps each journal line intact.
pub fn append_json_line_fast<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }

    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(&line)?;
    Ok(())
}

#[cfg(test)]
mod home_isolation_tests {
    use super::*;

    /// The layouts cargo actually produces, captured from a real toolchain:
    /// a harness binary lands in `deps/` with a metadata hash, while `cargo
    /// run`, a direct execution, and an installed binary never do.
    #[test]
    fn classifies_cargo_layouts_from_observed_paths() {
        for path in [
            "/private/tmp/probe_t/target/debug/deps/probe_t-a39295051af45621",
            "/repo/target/debug/deps/jcode_app_core-0f868b93b8de0cac",
            "/repo/target/selfdev/deps/integration_test-0123456789abcdef",
        ] {
            assert!(
                is_cargo_test_binary_path(Path::new(path)),
                "expected test-harness layout: {path}"
            );
        }

        for path in [
            // `cargo run` and direct execution: sibling of `deps`, not inside.
            "/private/tmp/probe_t/target/debug/probe_t",
            "/repo/target/debug/jcode",
            "/repo/target/release/jcode",
            // Installed binaries.
            "/usr/local/bin/jcode",
            "/Users/someone/.jcode/current/jcode",
            "/nix/store/abcdef-jcode-0.1.0/bin/jcode",
            // A `deps` directory that is not cargo's: no metadata hash.
            "/home/someone/deps/jcode",
            "/home/someone/deps/jcode-1.2.3",
            // Hash-shaped but too short to be cargo metadata.
            "/repo/target/debug/deps/jcode-abc123",
            // Non-hex suffix.
            "/repo/target/debug/deps/jcode-zzzzzzzzzzzzzzzz",
            // Empty binary name.
            "/repo/target/debug/deps/-0123456789abcdef",
        ] {
            assert!(
                !is_cargo_test_binary_path(Path::new(path)),
                "expected NOT a test-harness layout: {path}"
            );
        }
    }

    /// The guard must not fire when the caller pinned `JCODE_HOME`, and this
    /// very test binary must be classified as a harness (self-referential
    /// check: if the classifier regresses, this fails in the suite it guards).
    #[test]
    fn this_test_binary_is_classified_as_a_harness() {
        let exe = std::env::current_exe().expect("current_exe");
        assert!(
            is_cargo_test_binary_path(&exe),
            "this test binary should match the harness layout: {}",
            exe.display()
        );
    }

    /// `jcode_dir()` must never resolve to the real `~/.jcode` from a test.
    ///
    /// Asserts against `test_harness_home()` rather than by clearing
    /// `JCODE_HOME`: mutating that variable would race every concurrently
    /// running test through the global config-cache fingerprint, which is the
    /// exact defect class `scripts/check_config_env_lease.py` gates. The
    /// redirect is what needs proving, and it is observable directly.
    #[test]
    fn test_harness_home_is_never_the_real_home() {
        let redirected = test_harness_home().expect("test binaries must redirect");
        let real_home = dirs::home_dir().map(|home| home.join(".jcode"));

        assert_ne!(
            Some(&redirected),
            real_home.as_ref(),
            "a test resolved the developer's real jcode home"
        );
        assert!(
            redirected.starts_with(std::env::temp_dir()),
            "the redirect must land under the temp dir, got {}",
            redirected.display()
        );
        assert!(
            redirected.is_dir(),
            "the redirect target must exist: {}",
            redirected.display()
        );
    }
}
