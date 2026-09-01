//! Cross-process serialization for background status read-modify-write cycles.
//!
//! The lock target is a stable per-task sidecar because atomic status writes
//! replace the status file by rename. Locking the status file's own inode would
//! therefore stop coordinating writers after the first replacement. Old
//! binaries do not participate in this advisory lock, so a reload overlap with
//! a pre-lock writer remains an accepted non-guarantee.

use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub(crate) struct StatusFileLock {
    file: File,
}

pub(crate) fn lock_path(status_path: &Path) -> PathBuf {
    status_path.with_extension("lock")
}

fn open_lock_file(status_path: &Path) -> Result<File> {
    let path = lock_path(status_path);
    OpenOptions::new()
        .create(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("open background status lock {}", path.display()))
}

pub(crate) fn acquire_blocking(status_path: &Path) -> Result<StatusFileLock> {
    let file = open_lock_file(status_path)?;
    file.lock_exclusive().with_context(|| {
        format!(
            "lock background status {}",
            lock_path(status_path).display()
        )
    })?;
    Ok(StatusFileLock { file })
}

pub(crate) fn try_acquire(status_path: &Path) -> Result<Option<StatusFileLock>> {
    let file = open_lock_file(status_path)?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(StatusFileLock { file })),
        Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(anyhow::Error::from(error)).with_context(|| {
            format!(
                "try lock background status {}",
                lock_path(status_path).display()
            )
        }),
    }
}

impl Drop for StatusFileLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_path_replaces_json_extension() {
        assert_eq!(
            lock_path(Path::new("/tmp/task.status.json")),
            PathBuf::from("/tmp/task.status.lock")
        );
    }

    #[test]
    fn try_acquire_observes_guard_lifetime() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let status_path = tmp.path().join("task.status.json");
        let guard = acquire_blocking(&status_path).expect("first lock");
        assert!(
            try_acquire(&status_path)
                .expect("contended try lock")
                .is_none()
        );
        drop(guard);
        assert!(
            try_acquire(&status_path)
                .expect("try lock after drop")
                .is_some()
        );
    }

    #[test]
    fn blocking_acquire_succeeds_after_holder_drops() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let status_path = tmp.path().join("task.status.json");
        let guard = acquire_blocking(&status_path).expect("first lock");
        let child_path = status_path.clone();
        let waiter =
            std::thread::spawn(move || acquire_blocking(&child_path).expect("waiter lock"));
        std::thread::sleep(std::time::Duration::from_millis(25));
        assert!(
            !waiter.is_finished(),
            "blocking acquire returned while held"
        );
        drop(guard);
        drop(waiter.join().expect("join waiter"));
    }
}
