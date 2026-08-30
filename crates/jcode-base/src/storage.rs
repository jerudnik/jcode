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
/// them can safely run concurrently. Waiting writers block new readers only
/// while the lock is quiescent (no active readers). Once a read generation is
/// active, new readers keep joining it even when a writer waits: read leases
/// live as long as their owning Agent, and one live agent's test can only
/// finish after building further agents, so barring those dependent readers
/// behind a queued writer deadlocks the whole process (observed as 90-minute
/// CI hangs in `client_lifecycle` tests). The writer instead gets priority
/// over readers that arrive at quiescence.
#[cfg(any(test, feature = "test-support"))]
#[derive(Default)]
struct TestEnvLockState {
    active_readers: usize,
    active_writer: bool,
    waiting_writers: usize,
}

/// Upper bound on waiting for a test env lease before panicking.
///
/// A blocked lease acquisition means a lease leaked or the admission rules
/// regressed. Panicking with the lock state turns what would otherwise be a
/// silent multi-hour CI hang into a fast, attributable test failure.
#[cfg(any(test, feature = "test-support"))]
const TEST_ENV_LOCK_DEADLINE: std::time::Duration = std::time::Duration::from_secs(120);

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

/// Thread-local writer ownership used only for writer reentrancy.
///
/// Fixture child leases retain `inner` directly, not this owner token. Once all
/// thread-bound writer guards are dropped, the thread can no longer reacquire
/// the writer through TLS even if escaped fixtures still retain exclusion.
#[cfg(any(test, feature = "test-support"))]
struct TestEnvWriteOwner {
    inner: Arc<TestEnvWriteLeaseInner>,
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
    owner: Arc<TestEnvWriteOwner>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
enum TestEnvFixtureLeaseInner {
    Read { _lease: TestEnvReadLease },
    WriterChild { _lease: Arc<TestEnvWriteLeaseInner> },
}

/// A transferable lease retained by a long-lived test fixture.
///
/// Outside an environment writer this is a normal shared read lease. When the
/// fixture is constructed synchronously on the thread that owns the writer, it
/// becomes a writer-child lease: it does not grant mutation or writer
/// reentrancy, but keeps exclusive exclusion active until the fixture drops.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub struct TestEnvFixtureLease {
    _inner: TestEnvFixtureLeaseInner,
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
        let inner = Arc::new(TestEnvWriteLeaseInner { lock });
        Self {
            owner: Arc::new(TestEnvWriteOwner { inner }),
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
    static TEST_ENV_WRITE_OWNER: RefCell<Weak<TestEnvWriteOwner>> = const { RefCell::new(Weak::new()) };
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
    TEST_ENV_WRITE_OWNER.with(|slot| {
        slot.borrow().upgrade().map(|owner| TestEnvWriteLease {
            owner,
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
    let deadline = std::time::Instant::now() + TEST_ENV_LOCK_DEADLINE;
    while state.active_writer || (state.waiting_writers > 0 && state.active_readers == 0) {
        let (next, timeout) = lock
            .changed
            .wait_timeout(state, TEST_ENV_LOCK_DEADLINE)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state = next;
        if timeout.timed_out() && std::time::Instant::now() >= deadline {
            panic!(
                "test env read lease not granted within {TEST_ENV_LOCK_DEADLINE:?} \
                 (active_readers={}, active_writer={}, waiting_writers={}); \
                 a lease is stuck or the lock admission rules regressed",
                state.active_readers, state.active_writer, state.waiting_writers
            );
        }
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
    let deadline = std::time::Instant::now() + TEST_ENV_LOCK_DEADLINE;
    while state.active_writer || state.active_readers > 0 {
        let (next, timeout) = lock
            .changed
            .wait_timeout(state, TEST_ENV_LOCK_DEADLINE)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state = next;
        if timeout.timed_out() && std::time::Instant::now() >= deadline {
            state.waiting_writers = state.waiting_writers.saturating_sub(1);
            lock.changed.notify_all();
            panic!(
                "test env write lease not granted within {TEST_ENV_LOCK_DEADLINE:?} \
                 (active_readers={}, active_writer={}, waiting_writers={}); \
                 a read lease is stuck or leaked",
                state.active_readers, state.active_writer, state.waiting_writers
            );
        }
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
    assert!(
        current_test_env_write_lease().is_none(),
        "cannot acquire a test environment read lease while this thread holds a write lease"
    );
    if let Some(lease) = current_test_env_read_lease() {
        return lease;
    }

    let lease = acquire_test_env_read(test_env_lock_inner());
    TEST_ENV_READ_LEASE.with(|slot| *slot.borrow_mut() = Arc::downgrade(&lease.inner));
    lease
}

/// Acquire a transferable lease for a long-lived test fixture.
///
/// A fixture built under a same-thread writer retains that writer's exclusion
/// without retaining its thread-local reentrancy capability. An escaped fixture
/// therefore keeps new readers and writers blocked, while the original thread
/// cannot silently reacquire writer ownership after its scoped guard drops.
#[cfg(any(test, feature = "test-support"))]
pub fn lock_test_env_fixture() -> TestEnvFixtureLease {
    if let Some(writer) = current_test_env_write_lease() {
        TestEnvFixtureLease {
            _inner: TestEnvFixtureLeaseInner::WriterChild {
                _lease: Arc::clone(&writer.owner.inner),
            },
        }
    } else {
        TestEnvFixtureLease {
            _inner: TestEnvFixtureLeaseInner::Read {
                _lease: lock_test_env_read(),
            },
        }
    }
}

/// Try to retain process-global environment access for opportunistic test work.
///
/// Same-thread writers receive a writer-child lease, and existing readers nest
/// normally. If another thread owns (or is waiting for) the writer, return
/// `None` instead of blocking. This is intended for best-effort background work
/// that may safely defer until the next opportunity; blocking there can
/// deadlock when the caller itself retains an escaped writer-child fixture.
#[cfg(any(test, feature = "test-support"))]
pub fn try_lock_test_env_fixture() -> Option<TestEnvFixtureLease> {
    if let Some(writer) = current_test_env_write_lease() {
        return Some(TestEnvFixtureLease {
            _inner: TestEnvFixtureLeaseInner::WriterChild {
                _lease: Arc::clone(&writer.owner.inner),
            },
        });
    }
    if let Some(lease) = current_test_env_read_lease() {
        return Some(TestEnvFixtureLease {
            _inner: TestEnvFixtureLeaseInner::Read { _lease: lease },
        });
    }

    let lock = test_env_lock_inner();
    let mut state = lock
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if state.active_writer || state.waiting_writers > 0 {
        return None;
    }
    state.active_readers += 1;
    drop(state);

    let lease = TestEnvReadLease::new(lock);
    TEST_ENV_READ_LEASE.with(|slot| *slot.borrow_mut() = Arc::downgrade(&lease.inner));
    Some(TestEnvFixtureLease {
        _inner: TestEnvFixtureLeaseInner::Read { _lease: lease },
    })
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
    assert!(
        current_test_env_read_lease().is_none(),
        "cannot acquire a test environment write lease while this thread holds a read lease"
    );

    let lease = acquire_test_env_write(test_env_lock_inner());
    TEST_ENV_WRITE_OWNER.with(|slot| *slot.borrow_mut() = Arc::downgrade(&lease.owner));
    lease
}

/// Backwards-compatible name for the exclusive test environment lease.
#[cfg(any(test, feature = "test-support"))]
pub fn lock_test_env() -> TestEnvWriteLease {
    lock_test_env_write()
}

/// Restore an environment variable when the guard drops.
///
/// Tests that redirect `JCODE_HOME` at a temporary directory must restore the
/// previous value on *every* exit path. Hand-rolled `set_var` / restore pairs
/// skip the restore when the body panics or returns early, which leaves the
/// rest of the process pointed at a deleted tempdir, or worse, leaves a later
/// test writing into the developer's real `~/.jcode`. That is not theoretical:
/// a TUI test constructed a real `AmbientManager` under a manual guard and
/// leaked scheduled items into the developer's live ambient queue, where they
/// sat undeliverable until someone read the file.
///
/// Pair this with [`lock_test_env`], which provides the mutual exclusion; this
/// guard only provides the restore.
#[cfg(any(test, feature = "test-support"))]
pub struct EnvVarGuard {
    key: std::ffi::OsString,
    prev: Option<std::ffi::OsString>,
}

#[cfg(any(test, feature = "test-support"))]
impl EnvVarGuard {
    /// Set `key` to `value` for the guard's lifetime.
    pub fn set(key: impl Into<std::ffi::OsString>, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let key = key.into();
        let prev = std::env::var_os(&key);
        jcode_core::env::set_var(&key, value);
        Self { key, prev }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(prev) => jcode_core::env::set_var(&self.key, prev),
            None => jcode_core::env::remove_var(&self.key),
        }
    }
}

#[cfg(test)]
mod tests;
