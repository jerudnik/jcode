use super::ACTIVE_DIAGRAMS_MAX;
use crate::DiagramInfo;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

/// Active diagrams for info widget display
/// Updated during markdown rendering, queried by info_widget_data()
static ACTIVE_DIAGRAMS: LazyLock<Mutex<Vec<ActiveDiagram>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Session scope that new registrations are stamped with, and that reads are
/// filtered by. `UNSCOPED` means no session is bound yet: registrations are
/// stamped unscoped and reads return everything, which is what debug, bench
/// and unit-test paths rely on.
///
/// This is a process-global atomic rather than a thread-local because
/// `render_mermaid_sized_internal` registers from the detached
/// `jcode-mermaid-deferred` worker, which would not inherit a thread-local.
static CURRENT_DIAGRAM_SCOPE: AtomicU64 = AtomicU64::new(UNSCOPED);

/// The scope stamped on registrations made before any session is bound.
pub const UNSCOPED: u64 = 0;

/// Ephemeral diagram preview for in-flight streaming markdown.
/// This should never persist once a streaming segment is committed.
static STREAMING_PREVIEW_DIAGRAM: LazyLock<Mutex<Option<ActiveDiagram>>> =
    LazyLock::new(|| Mutex::new(None));

/// Info about an active diagram (for info widget)
#[derive(Clone)]
struct ActiveDiagram {
    hash: u64,
    width: u32,
    height: u32,
    label: Option<String>,
    /// Session this diagram was registered under. See `CURRENT_DIAGRAM_SCOPE`.
    scope: u64,
}

/// Bind subsequent registrations and reads to `scope`.
///
/// Diagrams already registered under other scopes are retained, not dropped:
/// switching back to a previous session must re-reveal its diagrams without a
/// re-render, because the body cache reuses retained messages and their
/// diagrams would never re-register.
pub fn set_diagram_scope(scope: u64) {
    CURRENT_DIAGRAM_SCOPE.store(scope, Ordering::Relaxed);
}

/// The scope registrations are currently stamped with.
pub fn current_diagram_scope() -> u64 {
    CURRENT_DIAGRAM_SCOPE.load(Ordering::Relaxed)
}

thread_local! {
    /// Overrides `CURRENT_DIAGRAM_SCOPE` for registrations on this thread.
    static REGISTRATION_SCOPE_OVERRIDE: std::cell::Cell<Option<u64>> =
        const { std::cell::Cell::new(None) };
}

/// Run `f` with registrations attributed to `scope` on this thread.
///
/// The deferred render worker uses this so a render queued under session A
/// that lands after a switch to session B is still attributed to A. The
/// worker thread both sets the override and performs the registration, so a
/// thread-local is sufficient and keeps the render path's signatures clean.
pub fn with_registration_scope<R>(scope: u64, f: impl FnOnce() -> R) -> R {
    let _guard = RegistrationScopeGuard::new(scope);
    f()
}

/// Attributes registrations on this thread to `scope` until dropped.
///
/// The closure form above reads better at a call site that already has one;
/// the guard suits the deferred worker, whose render call is already nested
/// inside an aspect-ratio scope.
pub struct RegistrationScopeGuard(Option<u64>);

impl RegistrationScopeGuard {
    pub fn new(scope: u64) -> Self {
        Self(REGISTRATION_SCOPE_OVERRIDE.with(|cell| cell.replace(Some(scope))))
    }
}

impl Drop for RegistrationScopeGuard {
    fn drop(&mut self) {
        REGISTRATION_SCOPE_OVERRIDE.with(|cell| cell.set(self.0));
    }
}

/// The scope a registration on this thread is attributed to.
fn registration_scope() -> u64 {
    REGISTRATION_SCOPE_OVERRIDE
        .with(|cell| cell.get())
        .unwrap_or_else(current_diagram_scope)
}

/// True when an entry registered under `entry_scope` is visible while
/// `viewing_scope` is bound. An unbound viewer sees everything, so debug and
/// test paths that never bind a session are unaffected.
fn scope_visible(entry_scope: u64, viewing_scope: u64) -> bool {
    viewing_scope == UNSCOPED || entry_scope == UNSCOPED || entry_scope == viewing_scope
}

fn to_diagram_info(diagram: ActiveDiagram) -> DiagramInfo {
    DiagramInfo {
        hash: diagram.hash,
        width: diagram.width,
        height: diagram.height,
        label: diagram.label,
    }
}

fn to_active_diagram(diagram: DiagramInfo, scope: u64) -> ActiveDiagram {
    ActiveDiagram {
        hash: diagram.hash,
        width: diagram.width,
        height: diagram.height,
        label: diagram.label,
        scope,
    }
}

pub fn register_active_diagram(hash: u64, width: u32, height: u32, label: Option<String>) {
    register_active_diagram_in_scope(hash, width, height, label, registration_scope());
}

/// Register under an explicit scope rather than the currently bound one.
pub fn register_active_diagram_in_scope(
    hash: u64,
    width: u32,
    height: u32,
    label: Option<String>,
    scope: u64,
) {
    if let Ok(mut diagrams) = ACTIVE_DIAGRAMS.lock() {
        // Dedup within a scope only: the same diagram content rendered in two
        // sessions is two entries, so re-registering in one session cannot
        // steal the other session's entry.
        if let Some(pos) = diagrams
            .iter()
            .position(|d| d.hash == hash && d.scope == scope)
        {
            let mut existing = diagrams.remove(pos);
            existing.width = width;
            existing.height = height;
            if label.is_some() {
                existing.label = label;
            }
            diagrams.push(existing);
        } else {
            diagrams.push(ActiveDiagram {
                hash,
                width,
                height,
                label,
                scope,
            });
        }
        while diagrams.len() > ACTIVE_DIAGRAMS_MAX {
            diagrams.remove(0);
        }
    }
}

/// Register or replace the current streaming preview diagram.
pub fn set_streaming_preview_diagram(hash: u64, width: u32, height: u32, label: Option<String>) {
    if let Ok(mut preview) = STREAMING_PREVIEW_DIAGRAM.lock() {
        *preview = Some(ActiveDiagram {
            hash,
            width,
            height,
            label,
            scope: current_diagram_scope(),
        });
    }
}

/// Clear the current streaming preview diagram.
pub fn clear_streaming_preview_diagram() {
    if let Ok(mut preview) = STREAMING_PREVIEW_DIAGRAM.lock() {
        *preview = None;
    }
}

/// Get active diagrams for info widget display
pub fn get_active_diagrams() -> Vec<DiagramInfo> {
    let viewing_scope = current_diagram_scope();
    // A streaming preview belongs to the session that is streaming. The
    // session-change handler already clears it; the scope filter is the
    // backstop for any path that does not.
    let preview = STREAMING_PREVIEW_DIAGRAM
        .lock()
        .ok()
        .and_then(|preview| preview.clone())
        .filter(|preview| scope_visible(preview.scope, viewing_scope));
    let preview_hash = preview.as_ref().map(|d| d.hash);

    let mut out = Vec::new();
    if let Some(diagram) = preview {
        out.push(to_diagram_info(diagram));
    }

    let viewing_scope = current_diagram_scope();
    if let Ok(diagrams) = ACTIVE_DIAGRAMS.lock() {
        out.extend(
            diagrams
                .iter()
                .rev()
                .filter(|d| Some(d.hash) != preview_hash)
                .filter(|d| scope_visible(d.scope, viewing_scope))
                .cloned()
                .map(to_diagram_info),
        );
    }

    out
}

/// Snapshot active diagrams visible in the current scope (internal order) for
/// temporary overrides in tests/debug
pub fn snapshot_active_diagrams() -> Vec<DiagramInfo> {
    let viewing_scope = current_diagram_scope();
    ACTIVE_DIAGRAMS
        .lock()
        .ok()
        .map(|diagrams| {
            diagrams
                .iter()
                .filter(|d| scope_visible(d.scope, viewing_scope))
                .cloned()
                .map(to_diagram_info)
                .collect()
        })
        .unwrap_or_default()
}

/// Restore active diagrams from a snapshot, stamped with the current scope.
pub fn restore_active_diagrams(snapshot: Vec<DiagramInfo>) {
    let scope = current_diagram_scope();
    if let Ok(mut diagrams) = ACTIVE_DIAGRAMS.lock() {
        diagrams.clear();
        diagrams.extend(
            snapshot
                .into_iter()
                .map(|diagram| to_active_diagram(diagram, scope)),
        );
        while diagrams.len() > ACTIVE_DIAGRAMS_MAX {
            diagrams.remove(0);
        }
    }
}

/// Count of diagrams visible in the current scope (what the pinned pane shows).
pub fn active_diagram_count() -> usize {
    let viewing_scope = current_diagram_scope();
    ACTIVE_DIAGRAMS
        .lock()
        .ok()
        .map(|diagrams| {
            diagrams
                .iter()
                .filter(|d| scope_visible(d.scope, viewing_scope))
                .count()
        })
        .unwrap_or(0)
}

/// Count of diagrams registered across every scope, which is what
/// `ACTIVE_DIAGRAMS_MAX` bounds.
pub fn total_active_diagram_count() -> usize {
    match ACTIVE_DIAGRAMS.lock() {
        Ok(diagrams) => diagrams.len(),
        Err(_) => 0,
    }
}

/// Clear every active diagram in every scope.
pub fn clear_active_diagrams() {
    if let Ok(mut diagrams) = ACTIVE_DIAGRAMS.lock() {
        diagrams.clear();
    }
    clear_streaming_preview_diagram();
}
