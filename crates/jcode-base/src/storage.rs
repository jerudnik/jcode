#![cfg_attr(test, allow(clippy::items_after_test_module))]

pub use jcode_storage::*;

use anyhow::Result;
use serde::de::DeserializeOwned;
use std::path::Path;

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    jcode_storage::read_json_with_recovery_handler(path, |event| match event {
        jcode_storage::StorageRecoveryEvent::CorruptPrimary { path, error } => {
            crate::logging::warn(&format!(
                "Corrupt JSON at {}, trying backup: {}",
                path.display(),
                error
            ));
        }
        jcode_storage::StorageRecoveryEvent::RecoveredFromBackup { backup_path } => {
            crate::logging::info(&format!("Recovered from backup: {}", backup_path.display()));
        }
    })
}

#[cfg(any(test, feature = "test-support"))]
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::Rc,
    sync::{Arc, Condvar, Mutex, OnceLock, Weak},
};

#[cfg(any(test, feature = "test-support"))]
pub struct TestCurrentDirGuard {
    original: std::path::PathBuf,
}

#[cfg(any(test, feature = "test-support"))]
impl TestCurrentDirGuard {
    pub fn set(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let original = std::env::current_dir()?;
        std::env::set_current_dir(path)?;
        Ok(Self { original })
    }

    pub fn change_to(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        std::env::set_current_dir(path)
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for TestCurrentDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

/// Process-global test-environment lease state.
///
/// Environment variables and their caches are mutable process-global state.
/// Tests that mutate them need exclusive access, while tests that only read
/// them can safely run concurrently. Waiting writers block new readers so a
/// steady reader stream cannot starve a scoped environment writer.
#[cfg(any(test, feature = "test-support"))]
#[derive(Default)]
struct TestEnvLockState {
    active_readers: usize,
    active_writer: bool,
    waiting_writers: usize,
}

#[cfg(any(test, feature = "test-support"))]
struct TestEnvLockInner {
    state: Mutex<TestEnvLockState>,
    changed: Condvar,
}

#[cfg(any(test, feature = "test-support"))]
impl Default for TestEnvLockInner {
    fn default() -> Self {
        Self {
            state: Mutex::new(TestEnvLockState::default()),
            changed: Condvar::new(),
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
struct TestEnvReadLeaseInner {
    lock: Arc<TestEnvLockInner>,
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for TestEnvReadLeaseInner {
    fn drop(&mut self) {
        let mut state = self
            .lock
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert!(state.active_readers > 0);
        state.active_readers = state.active_readers.saturating_sub(1);
        self.lock.changed.notify_all();
    }
}

#[cfg(any(test, feature = "test-support"))]
struct TestEnvWriteLeaseInner {
    lock: Arc<TestEnvLockInner>,
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for TestEnvWriteLeaseInner {
    fn drop(&mut self) {
        let mut state = self
            .lock
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert!(state.active_writer);
        state.active_writer = false;
        self.lock.changed.notify_all();
    }
}

/// A shared lease of the process-global test environment.
///
/// This intentionally owns no `MutexGuard`, so it is `Send + Sync + 'static`
/// and can be retained by test fixtures or app-owned tasks. Cloning a lease
/// retains the same acquisition until its last clone is dropped.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub struct TestEnvReadLease {
    inner: Arc<TestEnvReadLeaseInner>,
}

/// An exclusive lease of the process-global test environment.
///
/// Writer reentrancy is tracked per thread. The `Rc` marker deliberately makes
/// this lease `!Send + !Sync`, preventing a writer acquired on one thread from
/// being moved while that thread can still reacquire it through thread-local
/// state. Cloning retains the same acquisition on the owning thread.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub struct TestEnvWriteLease {
    inner: Arc<TestEnvWriteLeaseInner>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[cfg(any(test, feature = "test-support"))]
impl TestEnvReadLease {
    fn new(lock: Arc<TestEnvLockInner>) -> Self {
        Self {
            inner: Arc::new(TestEnvReadLeaseInner { lock }),
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl TestEnvWriteLease {
    fn new(lock: Arc<TestEnvLockInner>) -> Self {
        Self {
            inner: Arc::new(TestEnvWriteLeaseInner { lock }),
            _not_send_or_sync: PhantomData,
        }
    }
}

/// Backwards-compatible name for the exclusive test-environment lease.
#[cfg(any(test, feature = "test-support"))]
pub type TestEnvLease = TestEnvWriteLease;

#[cfg(any(test, feature = "test-support"))]
fn test_env_lock_inner() -> Arc<TestEnvLockInner> {
    static ENV_LOCK: OnceLock<Arc<TestEnvLockInner>> = OnceLock::new();
    Arc::clone(ENV_LOCK.get_or_init(|| Arc::new(TestEnvLockInner::default())))
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static TEST_ENV_READ_LEASE: RefCell<Weak<TestEnvReadLeaseInner>> = const { RefCell::new(Weak::new()) };
    static TEST_ENV_WRITE_LEASE: RefCell<Weak<TestEnvWriteLeaseInner>> = const { RefCell::new(Weak::new()) };
}

#[cfg(any(test, feature = "test-support"))]
fn current_test_env_read_lease() -> Option<TestEnvReadLease> {
    TEST_ENV_READ_LEASE.with(|slot| {
        slot.borrow()
            .upgrade()
            .map(|inner| TestEnvReadLease { inner })
    })
}

#[cfg(any(test, feature = "test-support"))]
fn current_test_env_write_lease() -> Option<TestEnvWriteLease> {
    TEST_ENV_WRITE_LEASE.with(|slot| {
        slot.borrow().upgrade().map(|inner| TestEnvWriteLease {
            inner,
            _not_send_or_sync: PhantomData,
        })
    })
}

#[cfg(any(test, feature = "test-support"))]
fn acquire_test_env_read(lock: Arc<TestEnvLockInner>) -> TestEnvReadLease {
    let mut state = lock
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while state.active_writer || state.waiting_writers > 0 {
        state = lock
            .changed
            .wait(state)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
    state.active_readers += 1;
    drop(state);
    TestEnvReadLease::new(lock)
}

#[cfg(any(test, feature = "test-support"))]
fn acquire_test_env_write(lock: Arc<TestEnvLockInner>) -> TestEnvWriteLease {
    let mut state = lock
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.waiting_writers += 1;
    while state.active_writer || state.active_readers > 0 {
        state = lock
            .changed
            .wait(state)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
    state.waiting_writers = state.waiting_writers.saturating_sub(1);
    state.active_writer = true;
    drop(state);
    TestEnvWriteLease::new(lock)
}

/// Acquire a shared lease for a test that only reads environment-derived
/// configuration or auth state. Read leases nest on one thread. Acquiring one
/// while holding a write lease is rejected because a `Send` read lease could
/// otherwise be moved to another thread while the writer continues mutating
/// process-global state.
#[cfg(any(test, feature = "test-support"))]
pub fn lock_test_env_read() -> TestEnvReadLease {
    if current_test_env_write_lease().is_some() {
        panic!(
            "cannot acquire a test environment read lease while this thread holds a write lease"
        );
    }
    if let Some(lease) = current_test_env_read_lease() {
        return lease;
    }

    let lease = acquire_test_env_read(test_env_lock_inner());
    TEST_ENV_READ_LEASE.with(|slot| *slot.borrow_mut() = Arc::downgrade(&lease.inner));
    lease
}

/// Acquire an exclusive lease for a test that mutates process-global
/// environment variables or their caches. Writers nest on one thread. A read
/// lease may not be upgraded, because that would deadlock against another
/// reader and would hide an unsafe writer-after-reader test pattern.
#[cfg(any(test, feature = "test-support"))]
pub fn lock_test_env_write() -> TestEnvWriteLease {
    if let Some(lease) = current_test_env_write_lease() {
        return lease;
    }
    if current_test_env_read_lease().is_some() {
        panic!(
            "cannot acquire a test environment write lease while this thread holds a read lease"
        );
    }

    let lease = acquire_test_env_write(test_env_lock_inner());
    TEST_ENV_WRITE_LEASE.with(|slot| *slot.borrow_mut() = Arc::downgrade(&lease.inner));
    lease
}

/// Backwards-compatible name for the exclusive test environment lease.
#[cfg(any(test, feature = "test-support"))]
pub fn lock_test_env() -> TestEnvWriteLease {
    lock_test_env_write()
}

#[cfg(test)]
mod tests;
