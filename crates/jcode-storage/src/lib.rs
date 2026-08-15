use anyhow::Result;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

mod active_pids;

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    jcode_home: Option<PathBuf>,
    harness_home: Option<PathBuf>,
    real_home: Option<PathBuf>,
    runtime_dir: PathBuf,
}

impl RuntimePaths {
    pub fn current() -> Self {
        Self {
            jcode_home: jcode_home_override().map(PathBuf::from),
            harness_home: test_harness_home(),
            real_home: dirs::home_dir(),
            runtime_dir: resolve_runtime_dir(),
        }
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.runtime_dir.clone()
    }

    pub fn jcode_dir(&self) -> Option<PathBuf> {
        resolve_jcode_dir(
            self.jcode_home.as_deref(),
            self.harness_home.clone(),
            self.real_home.clone(),
        )
    }

    pub fn app_config_dir(&self) -> Result<PathBuf> {
        resolve_app_config_dir(
            self.jcode_home.as_deref(),
            self.harness_home.as_deref(),
            dirs::config_dir(),
        )
    }

    pub fn app_cache_dir(&self) -> Result<PathBuf> {
        resolve_app_cache_dir(
            self.jcode_home.as_deref(),
            self.harness_home.as_deref(),
            dirs::cache_dir(),
        )
    }

    pub fn durable_state_dir(&self) -> PathBuf {
        if let Ok(dir) = std::env::var("JCODE_RUNTIME_DIR") {
            return PathBuf::from(dir).join("durable-state");
        }
        match self.jcode_dir() {
            Some(dir) => dir.join("state"),
            None => self.runtime_dir().join("durable-state"),
        }
    }

    pub fn user_home_path(&self, relative: impl AsRef<Path>) -> Result<PathBuf> {
        resolve_user_home_path(
            relative.as_ref(),
            self.jcode_home.as_deref(),
            self.harness_home.as_deref(),
            dirs::home_dir(),
        )
    }

    pub fn user_home_path_opt(&self, relative: impl AsRef<Path>) -> Option<PathBuf> {
        let relative = relative.as_ref();
        assert!(
            !relative.is_absolute(),
            "user_home_path_opt expects a relative path, got {}",
            relative.display()
        );
        resolve_user_home_path_opt(
            relative,
            self.jcode_home.as_deref(),
            self.harness_home.as_deref(),
            dirs::home_dir(),
        )
    }

    pub fn sanitize_ambient_dir_override(&self, value: Option<OsString>) -> Option<PathBuf> {
        let path = PathBuf::from(value?);
        if self.home_is_redirected() {
            match dirs::home_dir() {
                Some(real_home) if path.starts_with(&real_home) => None,
                _ => Some(path),
            }
        } else {
            Some(path)
        }
    }

    pub fn home_is_redirected(&self) -> bool {
        self.jcode_home.is_some() || self.harness_home.is_some()
    }

    pub fn test_root(prefix: &str) -> tempfile::TempDir {
        match tempfile::Builder::new().prefix(prefix).tempdir() {
            Ok(dir) => dir,
            Err(_) => std::process::abort(),
        }
    }
}

/// Serializes tests that mutate the process environment.
///
/// One lock for the whole crate, not one per test module: `JCODE_HOME` is
/// process-global, so a private mutex per module only excludes that module
/// against itself and lets the others run concurrently. That is exactly the
/// race that made the launcher-dir tests flake in the `jcode` binary.
///
/// The richer `TestEnvWriteLease` lives in `jcode-base`, which sits *above*
/// this crate and so cannot be used here.
#[cfg(test)]
pub(crate) fn lock_test_env_write() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
    resolve_runtime_dir()
}

fn resolve_runtime_dir() -> PathBuf {
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
    jcode_dir_opt().ok_or_else(|| anyhow::anyhow!("No home directory"))
}

/// [`jcode_dir`] for callers that treat a missing home as "feature
/// unavailable" and have no error channel to report into.
///
/// This is the primitive and [`jcode_dir`] wraps it, rather than the reverse:
/// the only way this resolution fails is a missing home directory, which is
/// exactly what these callers mean by `None`, so there is no error to discard.
/// Stating that once here keeps ~20 call sites from each writing
/// `jcode_dir().ok()`, which reads like a swallowed error even though it isn't.
pub fn jcode_dir_opt() -> Option<PathBuf> {
    RuntimePaths::current().jcode_dir()
}

/// The `JCODE_HOME` override, with blank values rejected.
///
/// Every ambient root reads this variable, so the "is it actually set to
/// something" rule belongs in one place. A whitespace-only value is a
/// *relative* path: taken literally it puts the jcode home under the current
/// working directory, which is how a directory named "\t" ended up in the repo
/// root full of telemetry and session data. Three of the four roots had the
/// same defect, so filtering per root would have left it live somewhere.
///
/// Returns `None` when unset or blank, so callers fall through to the harness
/// home and then the real platform location.
fn jcode_home_override() -> Option<OsString> {
    std::env::var_os("JCODE_HOME")
        .filter(|value| !value.is_empty())
        .filter(|value| !value.to_string_lossy().trim().is_empty())
}

/// Pure resolution rule behind [`jcode_dir_opt`]. See [`resolve_app_config_dir`]
/// for why the ambient inputs are arguments.
fn resolve_jcode_dir(
    jcode_home: Option<&Path>,
    harness_home: Option<PathBuf>,
    real_home: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(path) = jcode_home {
        return Some(path.to_path_buf());
    }

    if let Some(dir) = harness_home {
        return Some(dir);
    }

    Some(real_home?.join(".jcode"))
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
    RuntimePaths::current()
        .jcode_dir()
        .map(|dir| dir.join("logs"))
        .ok_or_else(|| anyhow::anyhow!("No home directory"))
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
    RuntimePaths::current().durable_state_dir()
}

/// Resolve jcode's app-owned config directory.
///
/// Default location is the platform config dir + `jcode` (for example
/// `~/.config/jcode` on Linux). When `JCODE_HOME` is set, sandbox this under
/// `$JCODE_HOME/config/jcode` so self-dev/tests do not leak into the user's
/// real config directory.
///
/// Like [`jcode_dir`], an unset `JCODE_HOME` under a test harness redirects to
/// the per-process temp home instead of the developer's real config dir. The
/// platform config dir is a *second* ambient root, distinct from `~/.jcode`,
/// so isolating only [`jcode_dir`] left this half of the surface exposed:
/// `model_picker_usage.json` lives here, feeds the picker's sort key, and made
/// `test_model_picker_preserves_recommendation_priority_order` pass or fail
/// according to which models the developer had personally selected.
pub fn app_config_dir() -> Result<PathBuf> {
    RuntimePaths::current().app_config_dir()
}

/// Pure resolution rule behind [`app_config_dir`].
///
/// Takes its three ambient inputs as arguments so every branch is testable
/// without mutating process env. Mutating `JCODE_HOME` in a test would race
/// each concurrently running test through the global config-cache
/// fingerprint, which is the exact defect class
/// `scripts/check_config_env_lease.py` gates.
fn resolve_app_config_dir(
    jcode_home: Option<&Path>,
    harness_home: Option<&Path>,
    platform_config_dir: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(path) = jcode_home.or(harness_home) {
        return Ok(path.join("config").join("jcode"));
    }

    let config_dir =
        platform_config_dir.ok_or_else(|| anyhow::anyhow!("No config directory found"))?;
    Ok(config_dir.join("jcode"))
}

/// The platform cache directory for jcode (`~/.cache/jcode`,
/// `~/Library/Caches/jcode`), isolated on the same rule as [`app_config_dir`].
///
/// A fourth ambient root. It was missing from this module while three separate
/// crates resolved it themselves through `dirs::cache_dir()`, so mermaid
/// renders and LaTeX images were written into the developer's real cache during
/// tests. Cache contents are derived data, which is exactly why this is easy to
/// overlook and still wrong: a stale entry keyed on content the test wrote is a
/// cross-test channel, and the writes accumulate in real user state.
pub fn app_cache_dir() -> Result<PathBuf> {
    RuntimePaths::current().app_cache_dir()
}

/// Pure resolution rule behind [`app_cache_dir`]. See [`resolve_app_config_dir`]
/// for why the ambient inputs are arguments.
fn resolve_app_cache_dir(
    jcode_home: Option<&Path>,
    harness_home: Option<&Path>,
    platform_cache_dir: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(path) = jcode_home.or(harness_home) {
        return Ok(path.join("cache").join("jcode"));
    }

    let cache_dir =
        platform_cache_dir.ok_or_else(|| anyhow::anyhow!("No cache directory found"))?;
    Ok(cache_dir.join("jcode"))
}

/// Resolve a path under the user's home directory, but sandbox it under
/// `$JCODE_HOME/external/` when `JCODE_HOME` is set.
///
/// This keeps external provider auth files isolated during tests and sandboxed
/// runs without changing default on-disk locations for normal users.
///
/// Third ambient root, isolated on the same rule as [`jcode_dir`] and
/// [`app_config_dir`]: with `JCODE_HOME` unset under a test harness this
/// resolves under the per-process temp home. Without that, a test asking for
/// something like `.aws/credentials` reads the developer's real one, so the
/// suite's verdict depends on which providers the developer happens to have
/// configured.
pub fn user_home_path(relative: impl AsRef<Path>) -> Result<PathBuf> {
    RuntimePaths::current().user_home_path(relative)
}

/// [`user_home_path`] for callers that treat a missing home as "feature
/// unavailable" and have no error channel to report into.
///
/// Exists so those callers do not each write `user_home_path(..).ok()`, which
/// silently discards *both* of the failure modes below. Only the first is a
/// legitimate `None`:
///
/// - no home directory: genuinely absent, and the caller's `None` branch (skip
///   the optional config file, fall back to another location) is correct;
/// - a caller passed an absolute path: a programmer error that `.ok()` turns
///   into a silent wrong answer, since the caller's fallback then runs as if
///   the home were missing. That stays a panic, because it is a bug in the
///   call site rather than a property of the machine.
pub fn user_home_path_opt(relative: impl AsRef<Path>) -> Option<PathBuf> {
    RuntimePaths::current().user_home_path_opt(relative)
}

/// Filter an ambient directory override (`$XDG_CONFIG_HOME`, `%LOCALAPPDATA%`,
/// ...) so it cannot defeat a redirected home.
///
/// These variables are not derived from the home directory, so the redirect in
/// [`user_home_path`] does not cover them. Two cases have to stay distinct:
///
/// - the variable points somewhere neutral (a temp dir, a custom layout): honor
///   it, since ignoring it would break users with non-default setups and tests
///   that deliberately point it at a fixture;
/// - the variable points *inside the real user home* while the home is
///   redirected: drop it. This is the case Linux CI hits, where
///   `XDG_CONFIG_HOME=/home/runner/.config`, and honoring it let sandboxed runs
///   read the runner's real configs.
///
/// Returns `None` when the override must be ignored, so callers can fall
/// through to their home-relative default.
pub fn sanitize_ambient_dir_override(value: Option<OsString>) -> Option<PathBuf> {
    RuntimePaths::current().sanitize_ambient_dir_override(value)
}

/// Assert that `path` is *not* under the real user home.
///
/// The regression tests for ambient roots all need to say "this resolved
/// somewhere other than the developer's real home". They cannot assert a fixed
/// path, because the test-harness redirect target is a per-process random temp
/// dir. Each one was reaching for `dirs::home_dir()` itself, which meant four
/// copies of the idiom and four crates carrying a `dirs` dev-dependency that
/// the ambient-roots gate then had to be told to ignore -- weakening the very
/// gate they exist to support.
///
/// Exposing the *assertion* rather than the home directory keeps the escape
/// hatch unusable for anything else: there is no way to spell "give me the real
/// home" with this, only "check that you did not land in it".
///
/// Panics with the offending path when the check fails.
#[track_caller]
pub fn assert_redirected_away_from_real_home(path: &Path, what: &str) {
    let Some(real_home) = dirs::home_dir() else {
        return;
    };
    assert!(
        !path.starts_with(&real_home),
        "{what} escaped the test-harness redirect and resolved under the real \
         home: {}",
        path.display()
    );
}

/// Whether ambient home resolution is currently redirected away from the real
/// user home (by `JCODE_HOME` or the per-process test-harness home).
///
/// Callers that resolve a path from a platform environment variable rather than
/// from the home directory (Windows `%APPDATA%`, say) need this: they must keep
/// honoring that variable in production, but must *not* let it escape the
/// sandbox under a harness. Exposing the predicate keeps that decision in one
/// place instead of re-deriving `JCODE_HOME.is_some()` at each call site, which
/// is exactly the hand-rolled check that missed the harness home in the Cursor
/// auth path.
pub fn home_is_redirected() -> bool {
    // Must use the same blank-rejecting rule as the roots themselves. A bare
    // `is_some()` reports "redirected" for a blank `JCODE_HOME` that no root
    // actually honors, which would make callers sandbox a path while the real
    // resolution fell through to the developer's home.
    jcode_home_override().is_some() || test_harness_home().is_some()
}

/// Pure resolution rule behind [`user_home_path`]. See
/// [`resolve_app_config_dir`] for why the ambient inputs are arguments.
fn resolve_user_home_path(
    relative: &Path,
    jcode_home: Option<&Path>,
    harness_home: Option<&Path>,
    real_home: Option<PathBuf>,
) -> Result<PathBuf> {
    if relative.is_absolute() {
        anyhow::bail!(
            "user_home_path expects a relative path, got {}",
            relative.display()
        );
    }

    resolve_user_home_path_opt(relative, jcode_home, harness_home, real_home)
        .ok_or_else(|| anyhow::anyhow!("No home directory"))
}

/// [`resolve_user_home_path`] minus the absolute-path rejection, which is the
/// caller's contract rather than a property of the machine.
///
/// Split out so [`user_home_path_opt`] does not have to collapse a `Result`
/// with two distinct failure modes into one `None`: it asserts the contract
/// itself and reaches this, so a missing home stays the only `None`.
fn resolve_user_home_path_opt(
    relative: &Path,
    jcode_home: Option<&Path>,
    harness_home: Option<&Path>,
    real_home: Option<PathBuf>,
) -> Option<PathBuf> {
    debug_assert!(!relative.is_absolute());
    if let Some(path) = jcode_home.or(harness_home) {
        return Some(path.join("external").join(relative));
    }

    Some(real_home?.join(relative))
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

/// A bounded cross-process exclusive lock backed by a filesystem lock file.
///
/// The lock is released automatically when this value is dropped. Callers
/// should acquire it from a blocking thread when used by async code.
pub struct ExclusiveFileLock {
    file: File,
}

impl ExclusiveFileLock {
    pub fn acquire(path: &Path, timeout: Duration, retry_delay: Duration) -> Result<Self> {
        if let Some(parent) = path.parent() {
            ensure_dir(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        jcode_core::fs::set_permissions_owner_only(path)?;

        let started = Instant::now();
        loop {
            match fs2::FileExt::try_lock_exclusive(&file) {
                Ok(()) => return Ok(Self { file }),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if started.elapsed() >= timeout {
                        anyhow::bail!(
                            "timed out after {:.1}s waiting for lock {}",
                            timeout.as_secs_f64(),
                            path.display()
                        );
                    }
                    std::thread::sleep(retry_delay.max(Duration::from_millis(1)));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Drop for ExclusiveFileLock {
    fn drop(&mut self) {
        if let Err(error) = fs2::FileExt::unlock(&self.file) {
            eprintln!("failed to unlock credential lock file: {error}");
        }
    }
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
mod tests;
