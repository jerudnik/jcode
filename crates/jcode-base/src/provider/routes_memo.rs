//! Route-catalog memoization: what gets cached, for how long, and who waits.
//!
//! Split out of `provider/mod.rs` because it is a self-contained policy unit
//! (an entry type, its freshness inputs, a measured TTL, and a single-flight
//! build guard) that was making an already-oversized module larger.
//!
//! The invariant worth stating once: **generations, not the TTL, are what make
//! an entry correct.** Auth changes and catalog refreshes bump the generations
//! and invalidate every entry immediately, regardless of age. The TTL only
//! bounds drift from inputs that nothing signals, which is why it can safely be
//! sized from measurement rather than guessed conservatively.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use super::ModelRoute;

/// Memoized route catalog with the inputs that decide its freshness: build
/// time (TTL), the auth generation at build time (bumped by
/// `AuthStatus::invalidate_cache()` on login/logout/credential edits), and the
/// catalog generation (bumped by prefetch/refresh completions).
#[derive(Clone)]
pub(crate) struct RoutesMemoEntry {
    pub(super) built_at: std::time::Instant,
    pub(super) auth_generation: u64,
    pub(super) catalog_generation: u64,
    pub(super) routes: Vec<ModelRoute>,
    /// `listable_model_names_from_routes(&routes)`, cached because the
    /// non-chat-model heuristic string-scans every route name and callers
    /// (catalog snapshots) ask for names and routes together.
    pub(super) listable_models: Vec<String>,
}

/// Process-wide route-catalog memo shared across `MultiProvider` instances.
///
/// The shared server forks one `MultiProvider` per client connection, so a
/// per-instance memo cannot deduplicate the builds triggered by a burst of
/// simultaneous client spawns: every fresh fork still built its own catalog.
/// Catalog content is derived almost entirely from process-global state
/// (credential files, disk caches, config), so identical forks can share one
/// build. Instance-specific inputs (active provider/model/profile) are folded
/// into the memo key; anything not captured is bounded by the TTL and the
/// auth/catalog generations.
pub(super) static GLOBAL_ROUTES_MEMO: LazyLock<Mutex<HashMap<String, RoutesMemoEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Single-flight guard for catalog builds. During a client connect burst every
/// connection calls `model_routes()` at nearly the same instant; without this
/// they all miss the still-empty memo and build the same catalog in parallel
/// (a thundering herd that pegs every core).
pub(super) static GLOBAL_ROUTES_BUILD_LOCK: Mutex<()> = Mutex::new(());

/// Bumped whenever provider catalogs change out-of-band (prefetch completion,
/// forced catalog refresh, auth changes). Invalidates every shared memo entry.
pub(super) static CATALOG_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub(super) fn catalog_generation() -> u64 {
    CATALOG_GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

pub(super) fn bump_catalog_generation() {
    CATALOG_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Longest catalog build observed in this process, in milliseconds.
///
/// The TTL is derived from this rather than guessed, because a memo whose TTL
/// is shorter than the build it caches cannot amortize anything: it expires
/// before it is ever reused, and the rebuild lands on whatever interactive path
/// asked next. That is not hypothetical. Measured `[TIMING] model_routes` lines
/// from this machine's own logs, n=97: p50 3198ms, p90 3691ms, p99/max 17039ms,
/// with 59.8% of builds over the old fixed 3000ms TTL. The *median* build
/// already outlived its own cache entry.
pub(super) static OBSERVED_MAX_ROUTES_BUILD_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Floor for the memo TTL, used before any build has been timed.
///
/// Sized above the measured p90 (3691ms) so a warm rebuild is amortized from
/// the very first build, instead of after the process has learned the hard way.
pub(super) const ROUTES_MEMO_MIN_TTL: std::time::Duration = std::time::Duration::from_secs(30);

/// Ceiling, so a pathological one-off build cannot pin a stale catalog for the
/// rest of the process. Correctness does not depend on this bound: auth changes
/// and catalog refreshes bump the generations, which invalidate every entry
/// immediately regardless of TTL.
pub(super) const ROUTES_MEMO_MAX_TTL: std::time::Duration = std::time::Duration::from_secs(600);

/// Multiple of the worst observed build time to keep an entry for.
///
/// A cache is worth having when it is held meaningfully longer than it costs to
/// build, which argues for some multiple above 1 but does not by itself pick a
/// value. 3x is chosen deliberately over a larger factor: the TTL is the only
/// bound on inputs that change routes *without* bumping a generation, and while
/// the two known bump sites (auth change, catalog refresh) cover every path I
/// traced, I cannot prove that set is exhaustive. A smaller multiple shrinks the
/// blast radius of an unknown path while still amortizing the build across
/// several reuse windows. With the measured p99 of 17039ms this yields ~51s.
pub(super) const ROUTES_MEMO_TTL_BUILD_MULTIPLE: u32 = 3;

/// TTL for a memoized catalog, derived from the slowest build actually
/// observed. Always exceeds that build time, which is the property the old
/// fixed 3s constant violated.
pub(super) fn routes_memo_ttl() -> std::time::Duration {
    let observed_ms = OBSERVED_MAX_ROUTES_BUILD_MS.load(std::sync::atomic::Ordering::Relaxed);
    let derived = std::time::Duration::from_millis(observed_ms)
        .saturating_mul(ROUTES_MEMO_TTL_BUILD_MULTIPLE);
    derived.clamp(ROUTES_MEMO_MIN_TTL, ROUTES_MEMO_MAX_TTL)
}

/// Record a completed build so the TTL tracks reality on this machine instead
/// of a number someone guessed once.
pub(super) fn record_routes_build_duration(elapsed: std::time::Duration) {
    let ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    OBSERVED_MAX_ROUTES_BUILD_MS.fetch_max(ms, std::sync::atomic::Ordering::Relaxed);
}

/// Age every shared memo entry past the TTL without touching the generations,
/// reproducing the exact state that matters here: content still valid, but the
/// TTL expired, so the caller falls through to the build path. Tests that skip
/// this never reach the single-flight lock at all and silently prove nothing.
#[cfg(test)]
pub(super) fn expire_routes_memo_ttl_for_test() {
    let aged = aged_past_ttl();
    if let Ok(mut shared) = GLOBAL_ROUTES_MEMO.lock() {
        for entry in shared.values_mut() {
            entry.built_at = aged;
        }
    }
}

/// An instant far enough in the past that any TTL comparison treats it as
/// expired.
#[cfg(test)]
pub(super) fn aged_past_ttl() -> std::time::Instant {
    std::time::Instant::now()
        .checked_sub(ROUTES_MEMO_MAX_TTL * 2)
        .expect("instant far enough in the past")
}

use super::catalog_routes;
use super::{MultiProvider, Provider, listable_model_names_from_routes, pricing};

impl MultiProvider {
    /// Age this instance's memo too. Both the instance and shared memos are
    /// consulted before the build lock, so aging only one leaves a fast path
    /// that returns early.
    #[cfg(test)]
    pub(super) fn expire_instance_routes_memo_ttl_for_test(&self) {
        if let Ok(mut memo) = self.routes_memo.lock()
            && let Some(entry) = memo.as_mut()
        {
            entry.built_at = aged_past_ttl();
        }
    }

    /// Drop this instance's route-catalog memo. Use for changes that are
    /// captured by [`Self::routes_memo_key`] (model/provider/profile switches):
    /// the shared memo stays valid because those instances key differently.
    pub(super) fn invalidate_routes_memo(&self) {
        if let Ok(mut memo) = self.routes_memo.lock() {
            *memo = None;
        }
    }

    /// Drop every memoized catalog in the process. Use for changes that alter
    /// catalog *content* beyond the memo key: credential changes and catalog
    /// prefetch/refresh completions. Deliberately not called from set_model /
    /// set_active_provider, which run once per shared-server fork during
    /// connect bursts and would otherwise defeat the shared memo.
    pub(super) fn invalidate_routes_memo_globally(&self) {
        self.invalidate_routes_memo();
        bump_catalog_generation();
    }

    /// Key identifying the instance-specific state that feeds the route
    /// catalog. Two `MultiProvider` instances with equal keys (given equal
    /// auth/catalog generations) produce equivalent catalogs, so shared-server
    /// forks can reuse one build. The current model matters because the active
    /// OpenRouter model gets priority endpoint-refresh scheduling and detail
    /// annotations in the catalog; the configured-provider bitmap matters
    /// because each configured runtime contributes its own route family.
    pub(super) fn routes_memo_key(&self) -> String {
        let active = self.active_provider();
        let credential_mode = self.credential_mode();
        let profile = self
            .active_openai_compatible_profile
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .unwrap_or_default();
        let mut compat_profiles: Vec<String> = self
            .openai_compatible_profiles
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .keys()
            .cloned()
            .collect();
        compat_profiles.sort();
        let configured = [
            ("cl", self.claude_provider().is_some()),
            ("an", self.anthropic_provider().is_some()),
            ("oa", self.openai_provider().is_some()),
            ("co", self.copilot_provider().is_some()),
            ("ag", self.antigravity_provider().is_some()),
            ("ge", self.gemini_provider().is_some()),
            ("cu", self.cursor_provider().is_some()),
            ("be", self.bedrock_provider().is_some()),
            ("or", self.openrouter_provider().is_some()),
        ]
        .iter()
        .filter(|(_, present)| *present)
        .map(|(tag, _)| *tag)
        .collect::<Vec<_>>()
        .join(",");
        format!(
            "{}|{}|{}|{:?}|{}|{}|{}|{}",
            // Scope by home so sandboxes (tests, JCODE_HOME switches) never
            // share catalogs that were built from different credential files.
            std::env::var("JCODE_HOME").unwrap_or_default(),
            Self::provider_key(active),
            self.model(),
            credential_mode,
            profile,
            self.use_claude_cli,
            configured,
            compat_profiles.join(","),
        )
    }

    /// Return a fresh memoized catalog entry (routes + listable model names),
    /// building it at most once per TTL window per catalog-relevant state.
    ///
    /// Freshness is keyed on a short TTL plus the auth and catalog
    /// generations. Lookup order: this instance's memo, the process-wide
    /// shared memo (so shared-server forks reuse one build), then a
    /// single-flight build that followers wait on instead of duplicating.
    /// Newest memo entry whose *content* is still valid, ignoring TTL age.
    ///
    /// Generations, not the TTL, are what make an entry correct: auth changes
    /// and catalog refreshes bump them and invalidate every entry immediately.
    /// The TTL only bounds drift from inputs nothing signals. So when a build
    /// is already in flight, a generation-current entry is a truthful answer,
    /// and returning it beats blocking an interactive request for seconds.
    pub(super) fn generation_current_routes_memo_entry(
        &self,
        shared_key: &str,
    ) -> Option<RoutesMemoEntry> {
        let auth_generation = pricing::auth_pricing_generation();
        let catalog_gen = catalog_generation();
        let current = |entry: &RoutesMemoEntry| {
            entry.auth_generation == auth_generation && entry.catalog_generation == catalog_gen
        };
        if let Ok(memo) = self.routes_memo.lock()
            && let Some(entry) = memo.as_ref()
            && current(entry)
        {
            return Some(entry.clone());
        }
        let shared = GLOBAL_ROUTES_MEMO.lock().ok()?;
        let entry = shared.get(shared_key)?;
        current(entry).then(|| entry.clone())
    }

    pub(super) fn fresh_routes_memo_entry(&self) -> RoutesMemoEntry {
        let auth_generation = pricing::auth_pricing_generation();
        let catalog_gen = catalog_generation();
        let ttl = routes_memo_ttl();
        let fresh = |entry: &RoutesMemoEntry| {
            entry.auth_generation == auth_generation
                && entry.catalog_generation == catalog_gen
                && entry.built_at.elapsed() < ttl
        };

        // Fast path: this instance already built (or copied) a fresh catalog.
        if let Ok(memo) = self.routes_memo.lock()
            && let Some(entry) = memo.as_ref()
            && fresh(entry)
        {
            return entry.clone();
        }

        // Shared path: another instance with the same catalog-relevant state
        // (typically a fresh fork on the shared server) built one already.
        let shared_key = self.routes_memo_key();
        let try_shared = || -> Option<RoutesMemoEntry> {
            let shared = GLOBAL_ROUTES_MEMO.lock().ok()?;
            let entry = shared.get(&shared_key)?;
            if !fresh(entry) {
                return None;
            }
            let entry = entry.clone();
            if let Ok(mut memo) = self.routes_memo.lock() {
                *memo = Some(entry.clone());
            }
            Some(entry)
        };
        if let Some(entry) = try_shared() {
            return entry;
        }

        // Single-flight: serialize builds so a connect burst produces one
        // build and N-1 memo hits instead of N parallel builds.
        //
        // Never *queue* behind a build that is already running. A rebuild
        // measured at up to 17s inside `Subscribe` blocks the server's
        // sequential request loop, so a 27ms `GetHistory` waits behind it and
        // the client's watchdog fires while the server is perfectly healthy.
        // When a build is already in flight, a generation-current entry is
        // still the right answer to give: a catalog a few seconds stale is a
        // far smaller defect than a session that looks dead.
        let _build_guard = match GLOBAL_ROUTES_BUILD_LOCK.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                if let Some(stale) = self.generation_current_routes_memo_entry(&shared_key) {
                    return stale;
                }
                // Nothing to serve yet (first build in this process), so there
                // is no honest alternative to waiting for the leader.
                GLOBAL_ROUTES_BUILD_LOCK
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
            }
        };
        // Re-check after acquiring the lock: the leader that held it may have
        // just published exactly the entry this instance needs.
        if let Some(entry) = try_shared() {
            return entry;
        }

        let build_started = std::time::Instant::now();
        let routes = catalog_routes::multiprovider_model_routes(self);
        // Feed the measured cost back into the TTL, so the window this entry is
        // kept for is derived from how long it actually took to produce.
        record_routes_build_duration(build_started.elapsed());
        let entry = RoutesMemoEntry {
            built_at: std::time::Instant::now(),
            auth_generation,
            catalog_generation: catalog_gen,
            listable_models: listable_model_names_from_routes(&routes),
            routes,
        };
        if let Ok(mut memo) = self.routes_memo.lock() {
            *memo = Some(entry.clone());
        }
        if let Ok(mut shared) = GLOBAL_ROUTES_MEMO.lock() {
            // Tiny keyspace (active provider + model + profile); prune stale
            // entries opportunistically so it cannot grow unbounded.
            shared.retain(|_, existing| fresh(existing));
            shared.insert(shared_key, entry.clone());
        }
        entry
    }
}
