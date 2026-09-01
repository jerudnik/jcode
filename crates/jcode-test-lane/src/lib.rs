//! Machine-scoped serialization for full-workspace test runs.
//!
//! Holder metadata is display-only. A waiter may read it to explain who owns
//! the lane, but it must never delete, truncate, or bypass a lock because the
//! metadata looks stale. Only `flock` decides whether the lane is held.
//! Metadata is truncated and rewritten after every successful acquisition.
//! A crashed holder may leave stale text behind; the next holder overwrites it.
//!
//! Unix uses `flock`. Non-Unix platforms deliberately run unserialized rather
//! than using a lock-file-existence scheme that can stall forever after a crash.

use std::env;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
pub const DEFAULT_POLL: Duration = Duration::from_millis(500);
const HEARTBEAT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct LaneOptions {
    pub path: PathBuf,
    pub label: String,
    pub timeout: Option<Duration>,
    pub poll: Duration,
}

impl LaneOptions {
    pub fn from_env(
        label: impl Into<String>,
        timeout: Option<Duration>,
    ) -> Result<Self, LaneError> {
        Ok(Self {
            path: lock_path_from_env()?,
            label: sanitize_label(&label.into()),
            timeout,
            poll: DEFAULT_POLL,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Holder {
    pub pid: u32,
    pub label: String,
    pub start_unix: u64,
}

impl fmt::Display for Holder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pid {} ({}, started at Unix time {})",
            self.pid, self.label, self.start_unix
        )
    }
}

#[derive(Debug)]
pub enum LaneError {
    Io {
        context: &'static str,
        source: io::Error,
    },
    InvalidTimeout(String),
    MissingHome,
    Timeout {
        holder: Option<Holder>,
        elapsed: Duration,
    },
}

impl fmt::Display for LaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { context, source } => write!(f, "workspace test lane: {context}: {source}"),
            Self::InvalidTimeout(value) => write!(
                f,
                "workspace test lane: invalid JCODE_TEST_LANE_TIMEOUT value {value:?}"
            ),
            Self::MissingHome => write!(f, "workspace test lane: could not resolve ~/.jcode"),
            Self::Timeout { holder, elapsed } => {
                write!(
                    f,
                    "workspace test lane: timed out after {} waiting for ",
                    format_elapsed(*elapsed)
                )?;
                match holder {
                    Some(holder) => write!(f, "{holder}")?,
                    None => write!(f, "an unknown holder")?,
                }
                write!(
                    f,
                    ". Run crate-scoped tests instead of retrying the full workspace blindly."
                )
            }
        }
    }
}

impl std::error::Error for LaneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum Guard {
    Held(LockFile),
    Bypassed,
    Nested,
    Unsupported,
}

#[derive(Debug)]
pub struct LockFile {
    file: Option<File>,
    path: PathBuf,
}

impl LockFile {
    #[cfg(unix)]
    pub fn fd_is_cloexec(&self) -> Result<bool, LaneError> {
        use std::os::fd::AsRawFd;

        let file = self.file.as_ref().expect("held lock file is open");
        let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
        if flags < 0 {
            return Err(io_error(
                "could not read lock fd flags",
                io::Error::last_os_error(),
            ));
        }
        Ok(flags & libc::FD_CLOEXEC != 0)
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        // Keep the close-before-unlink order used by the existing self-dev lock.
        // It is required on platforms that cannot unlink an open file.
        self.file.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn lock_path_from_env() -> Result<PathBuf, LaneError> {
    if let Some(path) = env::var_os("JCODE_TEST_LANE_PATH").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    jcode_storage::jcode_dir()
        .map(|dir| dir.join("test-lane.lock"))
        .map_err(|_| LaneError::MissingHome)
}

pub fn timeout_from_env() -> Result<Option<Duration>, LaneError> {
    match env::var("JCODE_TEST_LANE_TIMEOUT") {
        Ok(value) => parse_timeout(&value),
        Err(env::VarError::NotPresent) => Ok(Some(DEFAULT_TIMEOUT)),
        Err(env::VarError::NotUnicode(value)) => Err(LaneError::InvalidTimeout(
            value.to_string_lossy().into_owned(),
        )),
    }
}

pub fn parse_timeout(value: &str) -> Result<Option<Duration>, LaneError> {
    let seconds = value
        .parse::<u64>()
        .map_err(|_| LaneError::InvalidTimeout(value.to_string()))?;
    Ok((seconds != 0).then(|| Duration::from_secs(seconds)))
}

pub fn acquire(opts: LaneOptions) -> Result<Guard, LaneError> {
    acquire_with_writer(opts, &mut io::stderr())
}

pub fn acquire_with_writer(opts: LaneOptions, notes: &mut dyn Write) -> Result<Guard, LaneError> {
    if env::var("JCODE_TEST_LANE").is_ok_and(|value| value == "0") {
        return Ok(Guard::Bypassed);
    }
    if env::var_os("JCODE_TEST_LANE_HELD").is_some_and(|value| !value.is_empty()) {
        return Ok(Guard::Nested);
    }

    acquire_platform(opts, notes)
}

#[cfg(unix)]
fn acquire_platform(opts: LaneOptions, notes: &mut dyn Write) -> Result<Guard, LaneError> {
    use std::os::fd::AsRawFd;

    if let Some(parent) = opts.path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| io_error("could not create lock directory", err))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&opts.path)
        .map_err(|err| io_error("could not open lock file", err))?;
    set_cloexec(&file)?;

    let started = Instant::now();
    let mut emitted = false;
    let mut last_holder = None;
    let mut last_emitted = None;
    loop {
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            write_holder(&mut file, &opts.label)?;
            return Ok(Guard::Held(LockFile {
                file: Some(file),
                path: opts.path,
            }));
        }

        let err = io::Error::last_os_error();
        if !is_would_block(&err) {
            return Err(io_error("could not acquire lock", err));
        }

        let elapsed = started.elapsed();
        let holder = read_holder(&opts.path);
        let heartbeat_due = last_emitted.is_none_or(|at: Instant| at.elapsed() >= HEARTBEAT);
        if !emitted || holder != last_holder || heartbeat_due {
            let note = waiting_note(holder.as_ref(), elapsed);
            writeln!(notes, "{note}")
                .and_then(|()| notes.flush())
                .map_err(|err| io_error("could not write wait status", err))?;
            emitted = true;
            last_holder = holder.clone();
            last_emitted = Some(Instant::now());
        }

        if opts.timeout.is_some_and(|timeout| elapsed >= timeout) {
            return Err(LaneError::Timeout { holder, elapsed });
        }
        std::thread::sleep(opts.poll);
    }
}

#[cfg(not(unix))]
fn acquire_platform(_opts: LaneOptions, notes: &mut dyn Write) -> Result<Guard, LaneError> {
    writeln!(
        notes,
        "workspace test lane: unsupported on this platform; running unserialized"
    )
    .and_then(|()| notes.flush())
    .map_err(|err| io_error("could not write unsupported-platform status", err))?;
    Ok(Guard::Unsupported)
}

pub fn read_holder(path: &Path) -> Option<Holder> {
    let mut contents = String::new();
    File::open(path).ok()?.read_to_string(&mut contents).ok()?;
    let mut lines = contents.lines();
    let pid = lines.next()?.parse().ok()?;
    let label = lines.next()?.to_string();
    let start_unix = lines.next()?.parse().ok()?;
    Some(Holder {
        pid,
        label,
        start_unix,
    })
}

#[cfg(unix)]
pub fn lane_is_held(path: &Path) -> Result<bool, LaneError> {
    use std::os::fd::AsRawFd;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| io_error("could not create lock directory", err))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|err| io_error("could not open lock file", err))?;
    set_cloexec(&file)?;
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        let _ = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
        return Ok(false);
    }
    let err = io::Error::last_os_error();
    if is_would_block(&err) {
        Ok(true)
    } else {
        Err(io_error("could not inspect lock", err))
    }
}

#[cfg(not(unix))]
pub fn lane_is_held(_path: &Path) -> Result<bool, LaneError> {
    Ok(false)
}

#[cfg(unix)]
fn set_cloexec(file: &File) -> Result<(), LaneError> {
    use std::os::fd::AsRawFd;

    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io_error(
            "could not read lock fd flags",
            io::Error::last_os_error(),
        ));
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err(io_error(
            "could not set FD_CLOEXEC",
            io::Error::last_os_error(),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn is_would_block(err: &io::Error) -> bool {
    err.raw_os_error()
        .is_some_and(|code| code == libc::EWOULDBLOCK || code == libc::EAGAIN)
}

#[cfg(unix)]
fn write_holder(file: &mut File, label: &str) -> Result<(), LaneError> {
    let start_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    file.set_len(0)
        .map_err(|err| io_error("could not truncate holder metadata", err))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|err| io_error("could not seek holder metadata", err))?;
    write!(
        file,
        "{}\n{}\n{}\n",
        std::process::id(),
        sanitize_label(label),
        start_unix
    )
    .and_then(|()| file.flush())
    .map_err(|err| io_error("could not write holder metadata", err))
}

fn waiting_note(holder: Option<&Holder>, elapsed: Duration) -> String {
    match holder {
        Some(holder) => format!(
            "workspace test lane: waiting on lock held by {holder}; elapsed {}",
            format_elapsed(elapsed)
        ),
        None => format!(
            "workspace test lane: waiting on lock held by an unknown process; elapsed {}",
            format_elapsed(elapsed)
        ),
    }
}

fn format_elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{}m{:02}s", seconds / 60, seconds % 60)
}

fn sanitize_label(label: &str) -> String {
    let label = label.replace(['\n', '\r'], " ");
    let label = label.trim();
    if label.is_empty() {
        "workspace-test".to_string()
    } else {
        label.to_string()
    }
}

fn io_error(context: &'static str, source: io::Error) -> LaneError {
    LaneError::Io { context, source }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_zero_waits_forever() {
        assert_eq!(parse_timeout("0").unwrap(), None);
    }

    #[test]
    fn timeout_rejects_non_numeric_values() {
        assert!(matches!(
            parse_timeout("later"),
            Err(LaneError::InvalidTimeout(value)) if value == "later"
        ));
    }

    #[cfg(not(unix))]
    #[test]
    fn unsupported_platform_never_blocks() {
        let options = LaneOptions {
            path: PathBuf::from("unused"),
            label: "unsupported".to_string(),
            timeout: Some(Duration::ZERO),
            poll: Duration::ZERO,
        };
        let mut notes = Vec::new();
        assert!(matches!(
            acquire_platform(options, &mut notes).unwrap(),
            Guard::Unsupported
        ));
        assert!(
            String::from_utf8(notes)
                .unwrap()
                .contains("running unserialized")
        );
    }
}
