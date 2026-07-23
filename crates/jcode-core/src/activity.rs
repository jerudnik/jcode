//! Process-activity lease seam (F01 section 3.0, implemented by F02).
//!
//! This is the neutral lower-crate interface that lets code in `jcode-base`
//! (MCP manager/pool, background task manager) register "active work" with
//! the daemon's activity-lease authority without depending on server types.
//! The concrete authority lives in `jcode-app-core` and is injected downward
//! at composition roots as `Arc<dyn ActivityLeaseAuthority>`.
//!
//! Design contract: `docs/fork/ideal-base/evidence/F01/design.md` sections
//! 3.0-3.1. The trait is object-safe, synchronous, and std-only because
//! release must be callable from `Drop`.

use std::sync::Arc;

/// Work classes that hold activity leases (F01 section 3.0/3.3.3).
///
/// Detached background tasks, debug *connections*, headless session
/// *existence*, and parked persisted awaits deliberately never take a lease
/// (F01 section 1.1 non-lease census).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActivityClass {
    /// C1 carrier: one lease per counted client connection.
    ClientConnection,
    /// C1/C2/C3 turns: any streaming provider turn, any initiator.
    ProviderTurn,
    /// Bounded startup restoration window (F01 section 3.3.1).
    StartupRecovery,
    /// C4: async debug jobs started over the debug socket.
    DebugJob,
    /// C5: non-detached background tasks. C6 (detached) never leases.
    BackgroundTask,
    /// C7: any in-flight MCP call, pooled or per-session owned.
    McpCall,
    /// C8: live swarm await watcher tasks.
    SwarmWaiter,
    /// C9: due scheduled/ambient delivery dispatch gap.
    ScheduledDelivery,
}

impl ActivityClass {
    /// Stable label for logs and debug-socket attribution.
    pub fn as_str(self) -> &'static str {
        match self {
            ActivityClass::ClientConnection => "client-connection",
            ActivityClass::ProviderTurn => "provider-turn",
            ActivityClass::StartupRecovery => "startup-recovery",
            ActivityClass::DebugJob => "debug-job",
            ActivityClass::BackgroundTask => "background-task",
            ActivityClass::McpCall => "mcp-call",
            ActivityClass::SwarmWaiter => "swarm-waiter",
            ActivityClass::ScheduledDelivery => "scheduled-delivery",
        }
    }

    /// All lease classes, for exhaustive snapshots and tests.
    pub const ALL: [ActivityClass; 8] = [
        ActivityClass::ClientConnection,
        ActivityClass::ProviderTurn,
        ActivityClass::StartupRecovery,
        ActivityClass::DebugJob,
        ActivityClass::BackgroundTask,
        ActivityClass::McpCall,
        ActivityClass::SwarmWaiter,
        ActivityClass::ScheduledDelivery,
    ];
}

/// Opaque lease handle issued by an authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActivityLeaseToken(pub u64);

/// Typed acquisition refusal (F01 invariant I6: no intake after drain).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityLeaseError {
    /// The daemon has begun draining; new work must not start.
    ShuttingDown,
}

impl std::fmt::Display for ActivityLeaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivityLeaseError::ShuttingDown => {
                write!(f, "daemon is shutting down; new work refused")
            }
        }
    }
}

impl std::error::Error for ActivityLeaseError {}

/// Object-safe, synchronous lease authority.
///
/// Implementations must be cheap and non-blocking: `acquire`/`release` are
/// called from async contexts and from `Drop`.
pub trait ActivityLeaseAuthority: Send + Sync + 'static {
    fn acquire(
        &self,
        class: ActivityClass,
        label: &str,
    ) -> Result<ActivityLeaseToken, ActivityLeaseError>;
    fn release(&self, token: ActivityLeaseToken);
}

/// RAII lease guard: release on drop, so a panicked or aborted task can
/// never leak a lease (F01 invariant I3). Mirrors the existing
/// `OwnedChildPermit` pattern in the MCP client.
pub struct ActivityLeaseGuard {
    authority: Arc<dyn ActivityLeaseAuthority>,
    token: ActivityLeaseToken,
}

impl ActivityLeaseGuard {
    /// Acquire a lease, returning a guard that releases it on drop.
    pub fn acquire(
        authority: &Arc<dyn ActivityLeaseAuthority>,
        class: ActivityClass,
        label: &str,
    ) -> Result<Self, ActivityLeaseError> {
        let token = authority.acquire(class, label)?;
        Ok(Self {
            authority: Arc::clone(authority),
            token,
        })
    }

    /// The underlying token (for logging/attribution).
    pub fn token(&self) -> ActivityLeaseToken {
        self.token
    }
}

impl Drop for ActivityLeaseGuard {
    fn drop(&mut self) {
        self.authority.release(self.token);
    }
}

impl std::fmt::Debug for ActivityLeaseGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActivityLeaseGuard")
            .field("token", &self.token)
            .finish()
    }
}

/// No-op authority for tests and non-daemon binaries: always grants, never
/// tracks. Constructors in lower crates default to this so existing behavior
/// is unchanged unless a real authority is injected.
pub fn noop_activity_authority() -> Arc<dyn ActivityLeaseAuthority> {
    static NOOP: std::sync::OnceLock<Arc<dyn ActivityLeaseAuthority>> = std::sync::OnceLock::new();

    struct NoopAuthority;

    impl ActivityLeaseAuthority for NoopAuthority {
        fn acquire(
            &self,
            _class: ActivityClass,
            _label: &str,
        ) -> Result<ActivityLeaseToken, ActivityLeaseError> {
            Ok(ActivityLeaseToken(0))
        }

        fn release(&self, _token: ActivityLeaseToken) {}
    }

    Arc::clone(NOOP.get_or_init(|| Arc::new(NoopAuthority)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingAuthority {
        state: Mutex<RecordingState>,
    }

    #[derive(Default)]
    struct RecordingState {
        next: u64,
        active: Vec<u64>,
        refuse: bool,
    }

    impl ActivityLeaseAuthority for RecordingAuthority {
        fn acquire(
            &self,
            _class: ActivityClass,
            _label: &str,
        ) -> Result<ActivityLeaseToken, ActivityLeaseError> {
            let mut state = self.state.lock().unwrap();
            if state.refuse {
                return Err(ActivityLeaseError::ShuttingDown);
            }
            state.next += 1;
            let id = state.next;
            state.active.push(id);
            Ok(ActivityLeaseToken(id))
        }

        fn release(&self, token: ActivityLeaseToken) {
            let mut state = self.state.lock().unwrap();
            state.active.retain(|id| *id != token.0);
        }
    }

    #[test]
    fn guard_releases_on_drop() {
        let recording = Arc::new(RecordingAuthority::default());
        let authority: Arc<dyn ActivityLeaseAuthority> = recording.clone();

        let guard =
            ActivityLeaseGuard::acquire(&authority, ActivityClass::McpCall, "test").unwrap();
        assert_eq!(recording.state.lock().unwrap().active.len(), 1);
        drop(guard);
        assert!(recording.state.lock().unwrap().active.is_empty());
    }

    #[test]
    fn guard_releases_on_panic_unwind() {
        let recording = Arc::new(RecordingAuthority::default());
        let authority: Arc<dyn ActivityLeaseAuthority> = recording.clone();

        let authority_clone = Arc::clone(&authority);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _guard = ActivityLeaseGuard::acquire(
                &authority_clone,
                ActivityClass::ProviderTurn,
                "panicking",
            )
            .unwrap();
            panic!("simulated task panic");
        }));
        assert!(result.is_err());
        assert!(recording.state.lock().unwrap().active.is_empty());
    }

    #[test]
    fn refusal_is_typed() {
        let recording = Arc::new(RecordingAuthority::default());
        recording.state.lock().unwrap().refuse = true;
        let authority: Arc<dyn ActivityLeaseAuthority> = recording;

        let err = ActivityLeaseGuard::acquire(&authority, ActivityClass::DebugJob, "refused")
            .expect_err("acquisition must be refused");
        assert_eq!(err, ActivityLeaseError::ShuttingDown);
    }

    #[test]
    fn noop_always_grants() {
        let authority = noop_activity_authority();
        for class in ActivityClass::ALL {
            let guard = ActivityLeaseGuard::acquire(&authority, class, "noop").unwrap();
            drop(guard);
        }
    }
}
