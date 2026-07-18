//! Single bounded shutdown coordinator and server activity-lease authority.
//!
//! F02 implementation of the accepted F01 design
//! (`docs/fork/ideal-base/evidence/F01/design.md`, revision 4):
//!
//! - `ServerActivityLeaseAuthority`: the concrete `ActivityLeaseAuthority`
//!   over a pure `LeaseTable` (design 3.1). All work classes that must keep
//!   the daemon alive at `client_count == 0` hold RAII leases.
//! - `ShutdownCoordinator`: the one voluntary exit path (design 3.2). All
//!   exits converge on `begin(reason)`; phases are
//!   `Running -> Draining -> CleaningUp -> Cleaned` (or `Handoff` for
//!   reload). The executor NEVER calls `std::process::exit`: it publishes
//!   `Cleaned { reason, code }` and the top-level runner
//!   (`src/cli/dispatch.rs`) performs the one normal termination call.
//! - Coordinator-armed watchdog (design 3.2.4): the only other authorized
//!   termination site. `Cleaned` and `ForcedExit` are made mutually
//!   exclusive by an atomic Armed/Cancelled handoff.
//!
//! The authority and coordinator are process-global: the daemon is a
//! per-process singleton (enforced by the daemon lock). In-process test
//! servers share them (last-configured sidecar identity wins), which is the
//! same compromise the process-global reload channel already makes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use jcode_core::activity::{
    ActivityClass, ActivityLeaseAuthority, ActivityLeaseError, ActivityLeaseGuard,
    ActivityLeaseToken,
};

// ---------------------------------------------------------------------------
// Pure lease table (design 3.1)
// ---------------------------------------------------------------------------

struct LeaseEntry {
    class: ActivityClass,
    label: String,
    acquired_at: Instant,
}

/// Pure lease state: acquire/release/is_idle/refuse_new. No clocks beyond
/// caller-supplied instants, no process state.
#[derive(Default)]
pub(crate) struct LeaseTable {
    next_id: u64,
    active: HashMap<u64, LeaseEntry>,
    refusing: bool,
}

impl LeaseTable {
    fn acquire(
        &mut self,
        class: ActivityClass,
        label: &str,
        now: Instant,
    ) -> Result<u64, ActivityLeaseError> {
        if self.refusing {
            return Err(ActivityLeaseError::ShuttingDown);
        }
        self.next_id += 1;
        let id = self.next_id;
        self.active.insert(
            id,
            LeaseEntry {
                class,
                label: label.to_string(),
                acquired_at: now,
            },
        );
        Ok(id)
    }

    fn release(&mut self, id: u64) -> bool {
        self.active.remove(&id).is_some()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn is_idle(&self) -> bool {
        self.active.is_empty()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn count(&self) -> usize {
        self.active.len()
    }

    /// Attribution surface for `debug_socket` and F03 fixtures.
    #[allow(dead_code)]
    fn count_of(&self, class: ActivityClass) -> usize {
        self.active
            .values()
            .filter(|entry| entry.class == class)
            .count()
    }

    /// Drain-blocking work excludes client connections: connections are
    /// closed by intake shutdown, not waited for (design 4.1 C1 "abandon").
    fn drain_blocking_count(&self) -> usize {
        self.active
            .values()
            .filter(|entry| entry.class != ActivityClass::ClientConnection)
            .count()
    }

    fn refuse_new(&mut self) {
        self.refusing = true;
    }

    /// Atomic idle-shutdown claim (F02-B1): under the table lock, verify the
    /// table is COMPLETELY empty (client connections and drain-blocking work
    /// alike) and close acquisition in the same critical section. After a
    /// successful claim no lease of any class can be acquired, so quiescence
    /// cannot be lost between the idle decision and `begin`.
    fn try_claim_idle_shutdown(&mut self) -> bool {
        if !self.active.is_empty() {
            return false;
        }
        self.refusing = true;
        true
    }


    fn labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = self
            .active
            .values()
            .map(|entry| {
                format!(
                    "{}:{} ({}s)",
                    entry.class.as_str(),
                    entry.label,
                    entry.acquired_at.elapsed().as_secs()
                )
            })
            .collect();
        labels.sort();
        labels
    }
}

// ---------------------------------------------------------------------------
// Quiescence epoch (design 4.2)
// ---------------------------------------------------------------------------

/// One explicit quiescence epoch: `idle_since` is `None` whenever the system
/// is not fully quiescent, and is set only on the transition INTO quiescence.
/// A lease held past the timeout then released starts a full new window.
#[derive(Default)]
pub(crate) struct IdleClock {
    idle_since: Option<Instant>,
}

impl IdleClock {
    pub(crate) fn update(&mut self, quiescent: bool, now: Instant) {
        if quiescent {
            self.idle_since.get_or_insert(now);
        } else {
            self.idle_since = None;
        }
    }

    pub(crate) fn should_exit(&self, now: Instant, timeout: Duration) -> bool {
        matches!(self.idle_since, Some(since) if now.duration_since(since) >= timeout)
    }

    pub(crate) fn idle_elapsed(&self, now: Instant) -> Option<Duration> {
        self.idle_since.map(|since| now.duration_since(since))
    }
}

// ---------------------------------------------------------------------------
// Server activity-lease authority
// ---------------------------------------------------------------------------

fn lock_poisoned_ok<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) struct ServerActivityLeaseAuthority {
    table: Mutex<LeaseTable>,
}

impl ServerActivityLeaseAuthority {
    fn new() -> Self {
        Self {
            table: Mutex::new(LeaseTable::default()),
        }
    }

    pub(crate) fn client_connection_count(&self) -> usize {
        lock_poisoned_ok(&self.table).count_of(ActivityClass::ClientConnection)
    }

    pub(crate) fn drain_blocking_count(&self) -> usize {
        lock_poisoned_ok(&self.table).drain_blocking_count()
    }

    pub(crate) fn active_count(&self) -> usize {
        lock_poisoned_ok(&self.table).count()
    }

    pub(crate) fn active_labels(&self) -> Vec<String> {
        lock_poisoned_ok(&self.table).labels()
    }

    fn refuse_new(&self) {
        lock_poisoned_ok(&self.table).refuse_new();
    }

    fn try_claim_idle_shutdown(&self) -> bool {
        lock_poisoned_ok(&self.table).try_claim_idle_shutdown()
    }

}

impl ActivityLeaseAuthority for ServerActivityLeaseAuthority {
    fn acquire(
        &self,
        class: ActivityClass,
        label: &str,
    ) -> Result<ActivityLeaseToken, ActivityLeaseError> {
        lock_poisoned_ok(&self.table)
            .acquire(class, label, Instant::now())
            .map(ActivityLeaseToken)
    }

    fn release(&self, token: ActivityLeaseToken) {
        lock_poisoned_ok(&self.table).release(token.0);
    }
}

fn typed_authority() -> &'static Arc<ServerActivityLeaseAuthority> {
    static AUTHORITY: OnceLock<Arc<ServerActivityLeaseAuthority>> = OnceLock::new();
    AUTHORITY.get_or_init(|| Arc::new(ServerActivityLeaseAuthority::new()))
}

/// The process-global authority as the neutral trait object, for injection
/// into `jcode-base` composition roots (MCP manager/pool, background).
pub(crate) fn activity_authority() -> Arc<dyn ActivityLeaseAuthority> {
    Arc::clone(typed_authority()) as Arc<dyn ActivityLeaseAuthority>
}

/// Server-internal convenience: acquire a lease against the global authority.
/// A `ShuttingDown` refusal means new work must not start (invariant I6).
pub(crate) fn acquire_lease(
    class: ActivityClass,
    label: &str,
) -> Result<ActivityLeaseGuard, ActivityLeaseError> {
    ActivityLeaseGuard::acquire(&activity_authority(), class, label)
}

/// Typed snapshot access for the lifecycle monitors and debug surfaces.
pub(crate) fn lease_authority() -> &'static Arc<ServerActivityLeaseAuthority> {
    typed_authority()
}

// ---------------------------------------------------------------------------
// Debug fixture surface (F03)
// ---------------------------------------------------------------------------

/// Guards acquired via the debug socket, keyed by their lease token. Lets the
/// F03 runtime fixtures hold any lease class on a live daemon across debug
/// connections (debug connections themselves never count or lease), then
/// release and observe the idle exit.
fn debug_held_leases() -> &'static Mutex<HashMap<u64, ActivityLeaseGuard>> {
    static HELD: OnceLock<Mutex<HashMap<u64, ActivityLeaseGuard>>> = OnceLock::new();
    HELD.get_or_init(|| Mutex::new(HashMap::new()))
}

fn parse_activity_class(name: &str) -> Option<ActivityClass> {
    ActivityClass::ALL
        .into_iter()
        .find(|class| class.as_str() == name)
}

/// Acquire a lease of `class_name` on behalf of a debug fixture. Returns the
/// token, or a typed error string (unknown class / ShuttingDown).
pub(crate) fn debug_acquire_lease(class_name: &str) -> Result<u64, String> {
    let class = parse_activity_class(class_name).ok_or_else(|| {
        format!(
            "unknown lease class '{class_name}'; valid: {}",
            ActivityClass::ALL
                .iter()
                .map(|class| class.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let guard = acquire_lease(class, &format!("debug-fixture-{class_name}"))
        .map_err(|refused| refused.to_string())?;
    let token = guard.token().0;
    lock_poisoned_ok(debug_held_leases()).insert(token, guard);
    Ok(token)
}

/// Release a debug-held lease by token. Returns whether it existed.
pub(crate) fn debug_release_lease(token: u64) -> bool {
    lock_poisoned_ok(debug_held_leases()).remove(&token).is_some()
}

/// Snapshot for `shutdown:state`: coordinator phase, active leases, counts.
pub(crate) fn debug_shutdown_state() -> serde_json::Value {
    let coordinator = coordinator();
    let (phase, reason) = {
        let state = lock_poisoned_ok(&coordinator.state);
        (format!("{:?}", state.phase), state.reason.map(|r| r.as_str()))
    };
    serde_json::json!({
        "phase": phase,
        "reason": reason,
        "active_leases": lease_authority().active_count(),
        "drain_blocking_leases": lease_authority().drain_blocking_count(),
        "client_connection_leases": lease_authority().client_connection_count(),
        "lease_labels": lease_authority().active_labels(),
        "debug_held_tokens": lock_poisoned_ok(debug_held_leases()).keys().copied().collect::<Vec<_>>(),
    })
}

// ---------------------------------------------------------------------------
// Exit reasons (design 3.2.2, 3.2.5)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitReason {
    SigTerm,
    ReloadExecFailed,
    AcceptLoopFailure,
    Reload,
    TemporaryOwnerExit,
    TemporaryIdle,
    PersistentIdle,
}

impl ExitReason {
    /// Total priority lattice (design 3.2.2). Higher wins. The two idle
    /// reasons tie (a server is persistent xor temporary, so the tie is
    /// unreachable in practice; a tie is not an upgrade).
    pub(crate) fn priority(self) -> u8 {
        match self {
            ExitReason::SigTerm => 6,
            ExitReason::ReloadExecFailed => 5,
            ExitReason::AcceptLoopFailure => 4,
            ExitReason::Reload => 3,
            ExitReason::TemporaryOwnerExit => 2,
            ExitReason::TemporaryIdle | ExitReason::PersistentIdle => 1,
        }
    }

    /// Exit-code table (design 3.2.5). `Reload` has no code: handoff execs.
    pub(crate) fn exit_code(self) -> i32 {
        match self {
            ExitReason::SigTerm => 0,
            ExitReason::PersistentIdle
            | ExitReason::TemporaryIdle
            | ExitReason::TemporaryOwnerExit => super::EXIT_IDLE_TIMEOUT,
            ExitReason::ReloadExecFailed => 42,
            ExitReason::AcceptLoopFailure => EXIT_ACCEPT_LOOP_FAILURE,
            ExitReason::Reload => 0,
        }
    }

    /// Drain deadline per reason (design 3.2.3). Idle exits get zero by
    /// definition: quiescence was the precondition.
    pub(crate) fn drain_budget(self) -> Duration {
        match self {
            ExitReason::SigTerm
            | ExitReason::AcceptLoopFailure
            | ExitReason::TemporaryOwnerExit => Duration::from_secs(2),
            ExitReason::ReloadExecFailed => Duration::from_secs(1),
            ExitReason::PersistentIdle | ExitReason::TemporaryIdle => Duration::ZERO,
            // Bounded by the reload graceful-shutdown timeout.
            ExitReason::Reload => Duration::from_secs(5),
        }
    }

    /// Watchdog deadline: strictly above `drain_budget + CLEANUP_BUDGET`
    /// (invariant I5). Termination reasons only; reload handoff is not
    /// watchdog-armed (exec replaces the image; its failure upgrades to
    /// `ReloadExecFailed`, which is).
    pub(crate) fn watchdog_budget(self) -> Option<Duration> {
        match self {
            ExitReason::Reload => None,
            ExitReason::SigTerm => Some(Duration::from_secs(3)),
            _ => Some(self.drain_budget() + CLEANUP_BUDGET + Duration::from_secs(1)),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ExitReason::SigTerm => "sigterm",
            ExitReason::ReloadExecFailed => "reload-exec-failed",
            ExitReason::AcceptLoopFailure => "accept-loop-failure",
            ExitReason::Reload => "reload",
            ExitReason::TemporaryOwnerExit => "temporary-owner-exit",
            ExitReason::TemporaryIdle => "temporary-idle",
            ExitReason::PersistentIdle => "persistent-idle",
        }
    }
}

/// Distinct nonzero code for accept-loop failure (design 3.2.5).
pub(crate) const EXIT_ACCEPT_LOOP_FAILURE: i32 = 45;

/// Distinct code for a watchdog forced exit (design 3.2.5; EX_SOFTWARE).
pub(crate) const EXIT_FORCED: i32 = 70;

/// Total cleanup-phase budget; each step is individually bounded below this.
const CLEANUP_BUDGET: Duration = Duration::from_millis(700);

/// Per-step cleanup bound.
const CLEANUP_STEP_BUDGET: Duration = Duration::from_millis(250);

/// Poll interval while draining leases.
const DRAIN_POLL_INTERVAL: Duration = Duration::from_millis(50);

// ---------------------------------------------------------------------------
// Pure begin decision (design 3.2.2)
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BeginOutcome {
    /// This reason now drives (first begin, or an upgrade).
    Accepted,
    /// An equal-or-stronger reason already drives.
    SupersededBy(ExitReason),
    /// Typed refusal (e.g. reload on a temporary server).
    Refused(RefusalReason),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum RefusalReason {
    TemporaryServerNoReload,
    /// Idle begin refused: the lease table was not empty at claim time
    /// (F02-B1 atomic idle claim). The monitor keeps watching.
    NotQuiescent,
}

/// Pure upgrade decision: a strictly stronger reason upgrades; equal or
/// weaker is superseded. Upgrades re-derive the absolute deadline as
/// `min(current_deadline, now + full_budget(new))` (design 3.2.2).
pub(crate) fn decide_upgrade(current: ExitReason, requested: ExitReason) -> bool {
    requested.priority() > current.priority()
}

// ---------------------------------------------------------------------------
// Terminal outcomes (design 3.2.1)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalOutcome {
    /// Full cleanup ran; the process has NOT exited. The top-level runner
    /// performs the one normal termination call.
    Cleaned { reason: ExitReason, code: i32 },
}

// ---------------------------------------------------------------------------
// Watchdog: atomic Armed/Cancelled handoff (design 3.2.4)
// ---------------------------------------------------------------------------

const WATCHDOG_IDLE: u8 = 0;
const WATCHDOG_ARMED: u8 = 1;
const WATCHDOG_CANCELLED: u8 = 2;
const WATCHDOG_FIRING: u8 = 3;

struct Watchdog {
    state: AtomicU8,
    /// Absolute deadline in micros since `origin`. Only ever decreases while
    /// armed (upgrades never extend).
    deadline_micros: AtomicU64,
    origin: Instant,
}

impl Watchdog {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(WATCHDOG_IDLE),
            deadline_micros: AtomicU64::new(u64::MAX),
            origin: Instant::now(),
        }
    }

    fn deadline_from_now(&self, budget: Duration) -> u64 {
        (self.origin.elapsed() + budget).as_micros() as u64
    }

    /// Arm (or bring forward) the watchdog. Spawns the OS thread once.
    fn arm(self: &Arc<Self>, budget: Duration, reason: ExitReason) {
        let new_deadline = self.deadline_from_now(budget);
        // Never later than the previous deadline.
        self.deadline_micros
            .fetch_min(new_deadline, Ordering::SeqCst);
        if self
            .state
            .compare_exchange(
                WATCHDOG_IDLE,
                WATCHDOG_ARMED,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
        {
            record_forced_exit_marker("armed", reason);
            let watchdog = Arc::clone(self);
            if let Err(error) = std::thread::Builder::new()
                .name("jcode-shutdown-watchdog".into())
                .spawn({
                    let watchdog = Arc::clone(&watchdog);
                    move || watchdog.run()
                })
            {
                // F02-B5: without the OS-thread watchdog the I5 bound is
                // gone. Fail closed onto the tokio blocking pool, whose
                // workers are dedicated OS threads created lazily.
                crate::logging::error(&format!(
                    "Shutdown watchdog thread creation failed ({error}); \
                     falling back to the blocking pool."
                ));
                if tokio::runtime::Handle::try_current().is_ok() {
                    tokio::task::spawn_blocking(move || watchdog.run());
                } else {
                    crate::logging::error(
                        "Shutdown watchdog has NO thread; bounded exit cannot be guaranteed.",
                    );
                }
            }
        }
    }

    /// Executor-side decisive cancellation. Returns `true` if cancellation
    /// won (the watchdog can no longer fire); `false` if the watchdog
    /// already committed to firing, in which case `Cleaned` must NOT be
    /// published (design 3.2.4: mutual exclusion by atomic handoff).
    fn cancel(&self) -> bool {
        match self.state.compare_exchange(
            WATCHDOG_ARMED,
            WATCHDOG_CANCELLED,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {
                // F02-M1: record clean completion so a stale "armed" marker
                // cannot masquerade as a forced-exit post-mortem.
                record_forced_exit_marker("cancelled", current_reason_for_marker());
                true
            }
            // Never armed (reload handoff path): nothing to lose against.
            Err(state) => state == WATCHDOG_IDLE || state == WATCHDOG_CANCELLED,
        }
    }

    fn run(self: Arc<Self>) {
        loop {
            let now = self.origin.elapsed().as_micros() as u64;
            let deadline = self.deadline_micros.load(Ordering::SeqCst);
            if now < deadline {
                let sleep = Duration::from_micros((deadline - now).min(200_000));
                std::thread::sleep(sleep);
                continue;
            }
            // Deadline reached: attempt to claim the firing permit.
            match self.state.compare_exchange(
                WATCHDOG_ARMED,
                WATCHDOG_FIRING,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    crate::logging::warn(
                        "Shutdown watchdog fired: cleanup exceeded its deadline; forcing exit.",
                    );
                    record_forced_exit_marker("fired", current_reason_for_marker());
                    std::process::exit(EXIT_FORCED);
                }
                Err(_) => return, // Cancelled: executor won.
            }
        }
    }
}

/// Durable forced-exit marker (design 3.2.4): recorded when the watchdog is
/// armed and when it fires, so a post-mortem can see cleanup was preempted
/// even though the dying process cannot log afterwards.
fn record_forced_exit_marker(event: &str, reason: ExitReason) {
    let Ok(dir) = crate::storage::jcode_dir().map(|dir| dir.join("state")) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("shutdown-watchdog.json");
    let payload = serde_json::json!({
        "event": event,
        "reason": reason.as_str(),
        "pid": std::process::id(),
        "at": chrono::Utc::now().to_rfc3339(),
    });
    let _ = std::fs::write(&path, payload.to_string());
}

/// Spawn a future on the current tokio runtime when available, else on a
/// dedicated thread with a one-shot runtime (F02-I1: `begin` must not
/// silently strand `Draining` when called off-runtime, e.g. future callers
/// on plain OS threads).
fn spawn_on_runtime<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(future);
        }
        Err(_) => {
            std::thread::Builder::new()
                .name("jcode-shutdown-executor".into())
                .spawn(move || {
                    match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => runtime.block_on(future),
                        Err(error) => crate::logging::error(&format!(
                            "Shutdown executor runtime creation failed: {error}"
                        )),
                    }
                })
                .ok();
        }
    }
}

fn current_reason_for_marker() -> ExitReason {
    coordinator()
        .driving_reason()
        .unwrap_or(ExitReason::SigTerm)
}

// ---------------------------------------------------------------------------
// Coordinator (design 3.2)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Running,
    Draining,
    CleaningUp,
    Handoff,
    Terminal,
}

/// Sidecar identity and hooks the coordinator cleans up / cancels.
/// Configured by `Server::run` before any exit authority can fire.
/// Re-configurable (last write wins) for in-process test servers.
#[derive(Clone)]
pub(crate) struct ShutdownConfig {
    pub(crate) server_name: String,
    pub(crate) socket_path: PathBuf,
    pub(crate) debug_socket_path: PathBuf,
    pub(crate) temporary: bool,
    /// Cancelled at `Draining` start to stop intake (design 3.2.3).
    pub(crate) intake_cancel: Option<tokio_util::sync::CancellationToken>,
    /// MCP pool cell: shut down pool-wide during cleanup when initialized.
    pub(crate) mcp_pool:
        Option<Arc<tokio::sync::OnceCell<Arc<crate::mcp::SharedMcpPool>>>>,
}

struct CoordinatorState {
    phase: Phase,
    reason: Option<ExitReason>,
    /// Absolute drain deadline for the executing drain.
    drain_deadline: Option<Instant>,
    config: Option<ShutdownConfig>,
}

pub(crate) struct ShutdownCoordinator {
    state: Mutex<CoordinatorState>,
    terminal_tx: tokio::sync::watch::Sender<Option<TerminalOutcome>>,
    watchdog: Arc<Watchdog>,
    /// The lease authority this coordinator drains and claims against.
    /// The process-global coordinator uses the global authority; tests
    /// construct private instances with private authorities so real
    /// state-machine races can be driven in-process (F03).
    authority: Arc<ServerActivityLeaseAuthority>,
    /// Test instances disable the real watchdog: it calls `process::exit`
    /// and would kill the test binary. The watchdog atomic protocol has its
    /// own unit tests, and the real forced-exit path is proven by the
    /// process-level runtime fixture (exit code 70).
    watchdog_enabled: bool,
}

impl ShutdownCoordinator {
    fn new(authority: Arc<ServerActivityLeaseAuthority>) -> Self {
        let (terminal_tx, _) = tokio::sync::watch::channel(None);
        ShutdownCoordinator {
            state: Mutex::new(CoordinatorState {
                phase: Phase::Running,
                reason: None,
                drain_deadline: None,
                config: None,
            }),
            terminal_tx,
            watchdog: Arc::new(Watchdog::new()),
            authority,
            watchdog_enabled: true,
        }
    }

    /// Private leaked instance for state-machine tests (F03). The small
    /// intentional leak gives the `&'static self` the executor requires.
    #[cfg(test)]
    pub(crate) fn leaked_for_test() -> (&'static Self, Arc<ServerActivityLeaseAuthority>) {
        let authority = Arc::new(ServerActivityLeaseAuthority::new());
        let mut coordinator = Self::new(Arc::clone(&authority));
        coordinator.watchdog_enabled = false;
        let coordinator = Box::leak(Box::new(coordinator));
        (coordinator, authority)
    }

    /// Test observer: the current absolute drain deadline.
    #[cfg(test)]
    pub(crate) fn drain_deadline_for_test(&self) -> Option<Instant> {
        lock_poisoned_ok(&self.state).drain_deadline
    }

    /// Test observer: the driving reason.
    #[cfg(test)]
    pub(crate) fn driving_reason_for_test(&self) -> Option<ExitReason> {
        self.driving_reason()
    }
}

pub(crate) fn coordinator() -> &'static ShutdownCoordinator {
    static COORDINATOR: OnceLock<ShutdownCoordinator> = OnceLock::new();
    COORDINATOR.get_or_init(|| ShutdownCoordinator::new(Arc::clone(typed_authority())))
}

impl ShutdownCoordinator {
    /// Configure sidecar identity/hooks. Called by `Server::run` before any
    /// exit authority (idle monitors, SIGTERM handler, reload task) spawns.
    pub(crate) fn configure(&self, config: ShutdownConfig) {
        lock_poisoned_ok(&self.state).config = Some(config);
    }

    fn driving_reason(&self) -> Option<ExitReason> {
        lock_poisoned_ok(&self.state).reason
    }

    pub(crate) fn has_begun(&self) -> bool {
        lock_poisoned_ok(&self.state).phase != Phase::Running
    }

    /// Non-blocking begin (design 3.2.1). On first acceptance this task
    /// becomes the executor: it spawns the drain-cleanup future. Upgrades
    /// mutate the driving reason/deadline; the executing future observes
    /// them each poll (serialized executor by construction: only the first
    /// acceptance spawns).
    pub(crate) fn begin(&'static self, requested: ExitReason) -> BeginOutcome {
        let mut state = lock_poisoned_ok(&self.state);
        if requested == ExitReason::Reload
            && state.config.as_ref().is_some_and(|config| config.temporary)
        {
            // Temporary servers refuse reload (design 3.2.3 / I4).
            return BeginOutcome::Refused(RefusalReason::TemporaryServerNoReload);
        }
        let is_idle_reason = matches!(
            requested,
            ExitReason::PersistentIdle | ExitReason::TemporaryIdle
        );
        match state.phase {
            Phase::Running => {
                if is_idle_reason {
                    // F02-B1: idle exit must be claimed atomically. Under
                    // the lease-table lock, verify the table is completely
                    // empty AND close acquisition in one critical section.
                    // If any lease exists (client connection or work), the
                    // idle begin is refused and the monitor keeps watching.
                    if !self.authority.try_claim_idle_shutdown() {
                        return BeginOutcome::Refused(RefusalReason::NotQuiescent);
                    }
                } else {
                    // Close acquisition BEFORE the phase flips so no lease
                    // can slip in between decision and drain.
                    self.authority.refuse_new();
                }
                state.phase = Phase::Draining;
                state.reason = Some(requested);
                state.drain_deadline = Some(Instant::now() + requested.drain_budget());
                if self.watchdog_enabled
                    && let Some(budget) = requested.watchdog_budget()
                {
                    self.watchdog.arm(budget, requested);
                }
                drop(state);
                crate::logging::info(&format!(
                    "Shutdown coordinator: begin(reason={})",
                    requested.as_str()
                ));
                // Stop intake immediately (design 3.2.3).
                self.cancel_intake();
                // The accepting caller's context spawns the one executor
                // (falls back to a dedicated thread off-runtime, F02-I1).
                spawn_on_runtime(self.execute());
                BeginOutcome::Accepted
            }
            Phase::Draining => {
                let current = state.reason.expect("draining without reason");
                if decide_upgrade(current, requested) {
                    state.reason = Some(requested);
                    // Upgrades only shorten or preserve (design 3.2.2).
                    let upgraded = Instant::now() + requested.drain_budget();
                    state.drain_deadline = Some(match state.drain_deadline {
                        Some(existing) => existing.min(upgraded),
                        None => upgraded,
                    });
                    if self.watchdog_enabled
                        && let Some(budget) = requested.watchdog_budget()
                    {
                        self.watchdog.arm(budget, requested);
                    }
                    crate::logging::info(&format!(
                        "Shutdown coordinator: upgrade {} -> {}",
                        current.as_str(),
                        requested.as_str()
                    ));
                    BeginOutcome::Accepted
                } else {
                    BeginOutcome::SupersededBy(current)
                }
            }
            Phase::CleaningUp | Phase::Handoff | Phase::Terminal => {
                BeginOutcome::SupersededBy(state.reason.expect("post-drain without reason"))
            }
        }
    }

    /// Begin and await the terminal outcome of the whole shutdown
    /// (design 3.2.1). Returns the outcome regardless of which reason won.
    pub(crate) async fn begin_and_wait(
        &'static self,
        requested: ExitReason,
    ) -> Result<TerminalOutcome, RefusalReason> {
        if let BeginOutcome::Refused(refusal) = self.begin(requested) {
            return Err(refusal);
        }
        Ok(self.wait_terminal().await)
    }

    /// Observe the terminal outcome without requesting shutdown.
    pub(crate) async fn wait_terminal(&self) -> TerminalOutcome {
        let mut rx = self.terminal_tx.subscribe();
        loop {
            if let Some(outcome) = *rx.borrow_and_update() {
                return outcome;
            }
            if rx.changed().await.is_err() {
                std::future::pending::<()>().await;
            }
        }
    }

    fn cancel_intake(&self) {
        let token = lock_poisoned_ok(&self.state)
            .config
            .as_ref()
            .and_then(|config| config.intake_cancel.clone());
        if let Some(token) = token {
            token.cancel();
        }
    }

    /// The one executor future: drain, clean up, publish `Cleaned`.
    async fn execute(&'static self) {
        // Drain: wait for drain-blocking leases up to the (upgradable)
        // deadline. Client-connection leases are closed by intake
        // cancellation, not drained (design 4.1).
        loop {
            let (deadline, reason) = {
                let state = lock_poisoned_ok(&self.state);
                (
                    state.drain_deadline.expect("draining without deadline"),
                    state.reason.expect("draining without reason"),
                )
            };
            let blocking = self.authority.drain_blocking_count();
            if blocking == 0 {
                break;
            }
            if Instant::now() >= deadline {
                crate::logging::warn(&format!(
                    "Shutdown drain deadline reached (reason={}); abandoning {} lease(s): {}",
                    reason.as_str(),
                    blocking,
                    self.authority.active_labels().join(", ")
                ));
                break;
            }
            tokio::time::sleep(DRAIN_POLL_INTERVAL).await;
        }

        let (reason, config) = {
            let mut state = lock_poisoned_ok(&self.state);
            state.phase = Phase::CleaningUp;
            (
                state.reason.expect("cleanup without reason"),
                state.config.clone(),
            )
        };

        run_cleanup(reason, config.as_ref()).await;

        // Decisive watchdog cancellation BEFORE publishing `Cleaned`
        // (design 3.2.4). If the watchdog already committed to firing, the
        // process is dying with `ForcedExit`; `Cleaned` must never be
        // observed in that execution.
        if !self.watchdog.cancel() {
            crate::logging::warn(
                "Shutdown watchdog already firing; suppressing Cleaned publication.",
            );
            std::future::pending::<()>().await;
        }

        {
            let mut state = lock_poisoned_ok(&self.state);
            state.phase = Phase::Terminal;
        }
        let outcome = TerminalOutcome::Cleaned {
            reason,
            code: reason.exit_code(),
        };
        crate::logging::info(&format!(
            "Shutdown coordinator: cleanup complete (reason={}, code={})",
            reason.as_str(),
            reason.exit_code()
        ));
        // send_replace, not send: `watch::Sender::send` DROPS the value when
        // no receiver exists yet. A fast executor (empty table, quick
        // cleanup) can finish before any `wait_terminal` subscriber appears;
        // the outcome must still be stored or begin_and_wait callers hang
        // forever (found by the F03 pairwise race fixture).
        self.terminal_tx.send_replace(Some(outcome));
    }

    /// Reload drain entry (design 3.2.3 Handoff): the reload task calls this
    /// before its persistence/exec steps. Returns `Ok(())` when drain is
    /// complete and the phase is `Handoff`; the reload task then owns the
    /// handoff. If a stronger termination reason wins during the drain, the
    /// reload must abort and let the termination executor finish.
    pub(crate) async fn begin_reload_drain(&'static self) -> Result<(), ReloadRefused> {
        {
            let mut state = lock_poisoned_ok(&self.state);
            if state.config.as_ref().is_some_and(|config| config.temporary) {
                return Err(ReloadRefused::TemporaryServer);
            }
            match state.phase {
                Phase::Running => {
                    // Close lease acquisition BEFORE publishing the phase
                    // transition (F02-R2-I1, matching ordinary `begin`): no
                    // lease may be acquired during a reported Draining.
                    self.authority.refuse_new();
                    state.phase = Phase::Draining;
                    state.reason = Some(ExitReason::Reload);
                    state.drain_deadline =
                        Some(Instant::now() + ExitReason::Reload.drain_budget());
                }
                _ => {
                    return Err(ReloadRefused::ShutdownInProgress(
                        state.reason.expect("non-running without reason"),
                    ));
                }
            }
        }
        crate::logging::info("Shutdown coordinator: reload drain started");
        // F02-B3: reload Draining stops intake exactly like terminations
        // (design 3.2.3); accept loops must not admit new connections while
        // the handoff is being prepared.
        self.cancel_intake();

        loop {
            let (deadline, reason) = {
                let state = lock_poisoned_ok(&self.state);
                (
                    state.drain_deadline.expect("draining without deadline"),
                    state.reason.expect("draining without reason"),
                )
            };
            if reason != ExitReason::Reload {
                // A stronger termination reason upgraded us mid-drain; the
                // termination path owns completion now. But no executor was
                // spawned (reload drains inline), so spawn it here.
                spawn_on_runtime(self.execute());
                return Err(ReloadRefused::ShutdownInProgress(reason));
            }
            if self.authority.drain_blocking_count() == 0 || Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(DRAIN_POLL_INTERVAL).await;
        }

        let mut state = lock_poisoned_ok(&self.state);
        if state.reason != Some(ExitReason::Reload) {
            drop(state);
            spawn_on_runtime(self.execute());
            return Err(ReloadRefused::ShutdownInProgress(
                self.driving_reason().unwrap_or(ExitReason::SigTerm),
            ));
        }
        state.phase = Phase::Handoff;
        Ok(())
    }

    /// Reload exec failed (design 3.2.3): re-enter the termination path so
    /// the historic bare `exit(42)` finally gets full sidecar cleanup. The
    /// caller awaits `Cleaned { code: 42 }`; `run()` returns; the top-level
    /// runner exits.
    pub(crate) async fn reload_exec_failed(&'static self) -> TerminalOutcome {
        {
            let mut state = lock_poisoned_ok(&self.state);
            state.phase = Phase::Draining;
            state.reason = Some(ExitReason::ReloadExecFailed);
            state.drain_deadline =
                Some(Instant::now() + ExitReason::ReloadExecFailed.drain_budget());
        }
        if self.watchdog_enabled
            && let Some(budget) = ExitReason::ReloadExecFailed.watchdog_budget()
        {
            self.watchdog.arm(budget, ExitReason::ReloadExecFailed);
        }
        spawn_on_runtime(self.execute());
        self.wait_terminal().await
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum ReloadRefused {
    /// Typed refusal: temporary servers do not reload (design I4).
    TemporaryServer,
    /// A termination is already in progress; reload loses (design 3.2.2).
    ShutdownInProgress(ExitReason),
}

// ---------------------------------------------------------------------------
// Cleanup list (design 3.4): only real APIs, each step bounded.
// ---------------------------------------------------------------------------

/// Pure step plan, unit-testable (design 4.3).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn cleanup_step_plan(temporary: bool) -> Vec<&'static str> {
    let mut steps = vec![
        "unregister-registry",
        "remove-socket",
        "remove-debug-socket",
        "remove-hash-sidecar",
    ];
    if temporary {
        steps.push("remove-temporary-metadata");
    }
    steps.extend(["disconnect-mcp-pool", "finalize-background", "flush-log"]);
    steps
}

async fn bounded_step<F>(name: &str, step: F)
where
    F: std::future::Future<Output = ()>,
{
    if tokio::time::timeout(CLEANUP_STEP_BUDGET, step).await.is_err() {
        crate::logging::warn(&format!(
            "Shutdown cleanup step '{name}' exceeded its budget; continuing."
        ));
    }
}

async fn run_cleanup(reason: ExitReason, config: Option<&ShutdownConfig>) {
    // F03 forced-exit fixture injection: a deliberate cleanup hang lets the
    // fixture prove the coordinator-armed watchdog fires (exit 70, durable
    // marker) and that Cleaned is never published in that execution.
    if let Ok(value) = std::env::var("JCODE_TEST_SHUTDOWN_CLEANUP_HANG_MS")
        && let Ok(ms) = value.parse::<u64>()
        && ms > 0
    {
        crate::logging::warn(&format!(
            "TEST INJECTION: hanging shutdown cleanup for {ms}ms"
        ));
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    let Some(config) = config else {
        crate::logging::warn(
            "Shutdown coordinator: no config; skipping sidecar cleanup (test server?)",
        );
        return;
    };

    // 1. Unregister registry.
    bounded_step("unregister-registry", async {
        crate::registry::unregister_server_bounded(&config.server_name).await;
    })
    .await;

    // 2-3. Remove main + debug sockets.
    crate::transport::remove_socket(&config.socket_path);
    crate::transport::remove_socket(&config.debug_socket_path);

    // 4. Remove the `.hash` sidecar.
    let hash_path = format!("{}.hash", config.socket_path.display());
    let _ = std::fs::remove_file(&hash_path);

    // 5. Remove temporary metadata when temporary.
    if config.temporary {
        super::lifecycle::cleanup_temporary_metadata(&config.socket_path);
    }

    // 6. Shut down MCP children pool-wide.
    if let Some(cell) = config.mcp_pool.as_ref()
        && let Some(pool) = cell.get()
    {
        let pool = Arc::clone(pool);
        bounded_step("disconnect-mcp-pool", async move {
            pool.disconnect_all().await;
        })
        .await;
    }

    // 7. Finalize non-detached background statuses.
    bounded_step("finalize-background", async {
        let finalized = crate::background::global()
            .finalize_non_detached(reason.as_str())
            .await;
        if finalized > 0 {
            crate::logging::info(&format!(
                "Shutdown: finalized {finalized} non-detached background task(s)."
            ));
        }
    })
    .await;

    // 8. Flush the lifecycle log (best effort: emit the final record).
    crate::logging::info(&format!(
        "Shutdown cleanup complete (reason={})",
        reason.as_str()
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_with(classes: &[ActivityClass]) -> LeaseTable {
        let mut table = LeaseTable::default();
        for (index, class) in classes.iter().enumerate() {
            table
                .acquire(*class, &format!("t{index}"), Instant::now())
                .unwrap();
        }
        table
    }

    #[test]
    fn lease_table_acquire_release_idle() {
        let mut table = LeaseTable::default();
        assert!(table.is_idle());
        let id = table
            .acquire(ActivityClass::ProviderTurn, "turn", Instant::now())
            .unwrap();
        assert!(!table.is_idle());
        assert_eq!(table.count(), 1);
        assert!(table.release(id));
        assert!(table.is_idle());
        assert!(!table.release(id), "double release must be inert");
    }

    #[test]
    fn idle_claim_is_atomic_against_any_lease() {
        // F02-B1: the claim must fail while ANY lease exists (client
        // connections included) and must close acquisition on success.
        let mut table = table_with(&[ActivityClass::ClientConnection]);
        assert!(!table.try_claim_idle_shutdown(), "client connection blocks idle claim");
        let mut table = table_with(&[ActivityClass::McpCall]);
        assert!(!table.try_claim_idle_shutdown(), "work lease blocks idle claim");

        let mut table = LeaseTable::default();
        assert!(table.try_claim_idle_shutdown());
        let err = table
            .acquire(ActivityClass::ProviderTurn, "late", Instant::now())
            .expect_err("acquisition after a successful idle claim must fail");
        assert_eq!(err, ActivityLeaseError::ShuttingDown);
    }

    #[test]
    fn lease_table_refuses_after_drain_begins() {
        let mut table = LeaseTable::default();
        table.refuse_new();
        let err = table
            .acquire(ActivityClass::DebugJob, "late", Instant::now())
            .expect_err("acquire after refuse_new must fail");
        assert_eq!(err, ActivityLeaseError::ShuttingDown);
    }

    #[test]
    fn client_connections_are_not_drain_blocking() {
        let table = table_with(&[
            ActivityClass::ClientConnection,
            ActivityClass::ClientConnection,
        ]);
        assert_eq!(table.count(), 2);
        assert_eq!(table.drain_blocking_count(), 0);
        let table = table_with(&[ActivityClass::ClientConnection, ActivityClass::McpCall]);
        assert_eq!(table.drain_blocking_count(), 1);
    }

    #[test]
    fn idle_clock_quiescence_epoch() {
        // I1: a lease held past the timeout then released requires a FULL
        // new window (F01 design 4.2).
        let timeout = Duration::from_secs(300);
        let mut clock = IdleClock::default();
        let t0 = Instant::now();

        // Quiescent at t0: window starts.
        clock.update(true, t0);
        assert!(!clock.should_exit(t0 + Duration::from_secs(299), timeout));
        assert!(clock.should_exit(t0 + Duration::from_secs(300), timeout));

        // Lease acquired at t0+100 (non-quiescent): epoch resets to None.
        clock.update(false, t0 + Duration::from_secs(100));
        assert!(!clock.should_exit(t0 + Duration::from_secs(500), timeout));

        // Lease held past the timeout, then released at t0+500: full new
        // window required from t0+500.
        clock.update(true, t0 + Duration::from_secs(500));
        assert!(!clock.should_exit(t0 + Duration::from_secs(799), timeout));
        assert!(clock.should_exit(t0 + Duration::from_secs(800), timeout));
    }

    #[test]
    fn reason_lattice_is_total_and_strict() {
        use ExitReason::*;
        let order = [
            SigTerm,
            ReloadExecFailed,
            AcceptLoopFailure,
            Reload,
            TemporaryOwnerExit,
        ];
        for (i, stronger) in order.iter().enumerate() {
            for weaker in &order[i + 1..] {
                assert!(
                    decide_upgrade(*weaker, *stronger),
                    "{stronger:?} must upgrade over {weaker:?}"
                );
                assert!(
                    !decide_upgrade(*stronger, *weaker),
                    "{weaker:?} must not upgrade over {stronger:?}"
                );
            }
            assert!(
                !decide_upgrade(*stronger, *stronger),
                "{stronger:?} tie is not an upgrade"
            );
        }
        // Idle reasons tie with each other and lose to everything else.
        assert!(!decide_upgrade(PersistentIdle, TemporaryIdle));
        assert!(!decide_upgrade(TemporaryIdle, PersistentIdle));
        for reason in order {
            assert!(decide_upgrade(PersistentIdle, reason));
        }
    }

    #[test]
    fn exit_codes_match_design_table() {
        assert_eq!(ExitReason::SigTerm.exit_code(), 0);
        assert_eq!(ExitReason::PersistentIdle.exit_code(), 44);
        assert_eq!(ExitReason::TemporaryIdle.exit_code(), 44);
        assert_eq!(ExitReason::TemporaryOwnerExit.exit_code(), 44);
        assert_eq!(ExitReason::ReloadExecFailed.exit_code(), 42);
        assert_eq!(ExitReason::AcceptLoopFailure.exit_code(), 45);
        assert_eq!(EXIT_FORCED, 70);
    }

    #[test]
    fn watchdog_budget_exceeds_drain_plus_cleanup() {
        // Invariant I5: drain + cleanup < watchdog for every termination
        // reason. Reload (handoff) is not watchdog-armed.
        for reason in [
            ExitReason::SigTerm,
            ExitReason::ReloadExecFailed,
            ExitReason::AcceptLoopFailure,
            ExitReason::TemporaryOwnerExit,
            ExitReason::TemporaryIdle,
            ExitReason::PersistentIdle,
        ] {
            let watchdog = reason
                .watchdog_budget()
                .expect("termination reasons are watchdog-armed");
            assert!(
                reason.drain_budget() + CLEANUP_BUDGET < watchdog,
                "{reason:?}: drain {:?} + cleanup {CLEANUP_BUDGET:?} must be < watchdog {watchdog:?}",
                reason.drain_budget(),
            );
        }
        assert_eq!(ExitReason::Reload.watchdog_budget(), None);
    }

    #[test]
    fn watchdog_cancel_and_fire_are_mutually_exclusive() {
        // Executor wins: cancel succeeds, a later fire attempt must fail.
        let watchdog = Watchdog::new();
        watchdog.state.store(WATCHDOG_ARMED, Ordering::SeqCst);
        assert!(watchdog.cancel());
        assert!(
            watchdog
                .state
                .compare_exchange(
                    WATCHDOG_ARMED,
                    WATCHDOG_FIRING,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_err(),
            "cancelled watchdog must not claim the firing permit"
        );

        // Watchdog wins: firing claimed, executor cancel must lose.
        let watchdog = Watchdog::new();
        watchdog.state.store(WATCHDOG_FIRING, Ordering::SeqCst);
        assert!(!watchdog.cancel(), "executor must observe the lost race");

        // Never armed (reload path): cancel trivially succeeds.
        let watchdog = Watchdog::new();
        assert!(watchdog.cancel());
    }

    #[test]
    fn watchdog_deadline_only_decreases() {
        let watchdog = Watchdog::new();
        watchdog.deadline_micros.store(5_000_000, Ordering::SeqCst);
        watchdog
            .deadline_micros
            .fetch_min(3_000_000, Ordering::SeqCst);
        assert_eq!(watchdog.deadline_micros.load(Ordering::SeqCst), 3_000_000);
        // A later, larger deadline must not extend.
        watchdog
            .deadline_micros
            .fetch_min(9_000_000, Ordering::SeqCst);
        assert_eq!(watchdog.deadline_micros.load(Ordering::SeqCst), 3_000_000);
    }

    #[test]
    fn cleanup_plan_covers_design_step_list() {
        let persistent = cleanup_step_plan(false);
        assert_eq!(
            persistent,
            vec![
                "unregister-registry",
                "remove-socket",
                "remove-debug-socket",
                "remove-hash-sidecar",
                "disconnect-mcp-pool",
                "finalize-background",
                "flush-log",
            ]
        );
        let temporary = cleanup_step_plan(true);
        assert!(temporary.contains(&"remove-temporary-metadata"));
        assert_eq!(temporary.len(), persistent.len() + 1);
    }
}
