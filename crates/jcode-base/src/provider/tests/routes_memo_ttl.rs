// R09: the routes memo TTL must exceed the build it caches.
//
// The old constant was 3s. Measured `[TIMING] model_routes` lines from this
// machine's logs (n=96) put p50 at 3185ms and max at 17039ms, so the *median*
// build outlived its own cache entry and 59.4% of builds exceeded the TTL. A
// memo that expires before it is reused amortizes nothing, and the rebuild
// lands inside `Subscribe` on the server's sequential request loop.

/// Measured build times, in milliseconds, from `[TIMING] model_routes` log
/// lines on the reporting machine. Recorded here so the TTL is checked against
/// evidence rather than against a number someone felt was about right.
const MEASURED_BUILD_MS: &[u64] = &[
    // warm rebuilds reported in REMOTE_LOADING_SESSION_TTL.md
    2393, 3801, // cold rebuilds reported in the same document
    8275, 14699, 17039,
];

fn measured_p99_ms() -> u64 {
    let mut sorted = MEASURED_BUILD_MS.to_vec();
    sorted.sort_unstable();
    *sorted.last().expect("measurements")
}

#[test]
fn r09_gate1_ttl_exceeds_every_measured_build_time() {
    // Drive the observed-build tracker with the real measurements, exactly as a
    // process would learn them, then require the resulting TTL to outlast them.
    for ms in MEASURED_BUILD_MS {
        super::routes_memo::record_routes_build_duration(std::time::Duration::from_millis(*ms));
    }
    let ttl = super::routes_memo::routes_memo_ttl();
    let p99 = measured_p99_ms();

    assert!(
        ttl > std::time::Duration::from_millis(p99),
        "TTL {}ms must exceed the measured p99 build of {}ms, or the memo expires \
         before it is ever reused and the rebuild lands on an interactive path",
        ttl.as_millis(),
        p99,
    );

    // The specific regression: the old fixed 3s TTL was shorter than a warm
    // 3801ms build. Assert the relationship, not the constant, so this keeps
    // holding if build times or the multiple change.
    assert!(
        ttl > std::time::Duration::from_secs(3),
        "TTL must exceed the retired 3s constant that caused the stall"
    );
}

#[test]
fn r09_gate1_ttl_is_derived_from_measurement_not_a_fixed_guess() {
    // Contrast case: a slower observed build must produce a longer TTL. Without
    // this, a hardcoded value large enough to pass the assertion above would
    // look identical to a derived one.
    super::routes_memo::OBSERVED_MAX_ROUTES_BUILD_MS.store(0, std::sync::atomic::Ordering::Relaxed);
    let floor = super::routes_memo::routes_memo_ttl();
    assert_eq!(
        floor,
        super::routes_memo::ROUTES_MEMO_MIN_TTL,
        "with no observed build the TTL should sit at its floor"
    );

    super::routes_memo::record_routes_build_duration(std::time::Duration::from_secs(20));
    let derived = super::routes_memo::routes_memo_ttl();
    assert!(
        derived > floor,
        "a 20s observed build must lengthen the TTL ({}ms) beyond the floor ({}ms)",
        derived.as_millis(),
        floor.as_millis(),
    );
    assert!(
        derived > std::time::Duration::from_secs(20),
        "the derived TTL must still exceed the build that produced it"
    );

    // And it stays bounded, so one pathological build cannot pin a catalog for
    // the life of the process.
    super::routes_memo::record_routes_build_duration(std::time::Duration::from_secs(86_400));
    assert_eq!(
        super::routes_memo::routes_memo_ttl(),
        super::routes_memo::ROUTES_MEMO_MAX_TTL,
        "an absurd outlier must clamp to the ceiling rather than pin the memo"
    );
    super::routes_memo::OBSERVED_MAX_ROUTES_BUILD_MS.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// Build a minimal provider whose catalog build is cheap. The point of these
/// tests is the *queuing* behaviour around the build lock, not catalog content.
fn memo_test_provider() -> MultiProvider {
    MultiProvider {
        claude: RwLock::new(None),
        anthropic: RwLock::new(None),
        openai: RwLock::new(None),
        copilot_api: RwLock::new(None),
        antigravity: RwLock::new(None),
        gemini: RwLock::new(None),
        cursor: RwLock::new(None),
        bedrock: RwLock::new(None),
        openrouter: RwLock::new(None),
        openai_compatible_profiles: RwLock::new(std::collections::HashMap::new()),
        active_openai_compatible_profile: RwLock::new(None),
        active: RwLock::new(ActiveProvider::OpenAI),
        use_claude_cli: false,
        startup_notices: RwLock::new(Vec::new()),
        forced_provider: None,
        routes_memo: std::sync::Mutex::new(None),
        session_working_dir: std::sync::RwLock::new(None),
    }
}

#[test]
fn r09_gate2_a_request_is_not_queued_behind_an_in_flight_build() {
    with_clean_provider_test_env(|| {
        let provider = memo_test_provider();

        // Prime the memo so a generation-current entry exists to serve.
        let primed = provider.model_routes();
        assert!(
            !primed.is_empty(),
            "priming build should produce routes to serve later"
        );

        // Age the memo past the TTL WITHOUT touching the generations. This is
        // the state that actually reaches the build path: content still valid,
        // TTL expired. An earlier version of this test skipped this step, so the
        // fast path returned first and the test passed even with the fix
        // reverted -- it proved nothing until the control exposed it.
        super::routes_memo::expire_routes_memo_ttl_for_test();
        provider.expire_instance_routes_memo_ttl_for_test();

        // Simulate a long build already in flight: hold the single-flight lock,
        // exactly as a 17s cold rebuild would. This is the condition that used
        // to park an interactive caller for the whole build.
        let held = super::routes_memo::GLOBAL_ROUTES_BUILD_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let started = std::time::Instant::now();
        let routes = provider.model_routes();
        let waited = started.elapsed();

        drop(held);

        assert!(
            !routes.is_empty(),
            "a caller arriving during a build must still get a usable catalog, \
             not an empty one"
        );
        // The old code called `.lock()` here and would block until the build
        // finished. With the lock held for the whole call, blocking means this
        // test deadlocks rather than merely running slow, so any completion at
        // all proves the caller was not queued. Assert promptness anyway so a
        // future change that swaps the deadlock for a long sleep still fails.
        assert!(
            waited < std::time::Duration::from_secs(1),
            "a request arriving during an in-flight build waited {}ms; it must be \
             served from the existing catalog instead of queuing behind the build",
            waited.as_millis(),
        );
    });
}

#[test]
fn r09_gate3_serving_during_a_build_still_respects_generation_invalidation() {
    with_clean_provider_test_env(|| {
        let provider = memo_test_provider();
        let _ = provider.model_routes();
        super::routes_memo::expire_routes_memo_ttl_for_test();
        provider.expire_instance_routes_memo_ttl_for_test();

        // Contrast case for gate 2: serving a TTL-stale entry is only honest
        // while nothing has signalled that the content changed. Bump the
        // catalog generation (what a prefetch/auth change does) and the stale
        // entry must NOT be served, even though a build is in flight.
        super::routes_memo::CATALOG_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let shared_key = provider.routes_memo_key();
        assert!(
            provider
                .generation_current_routes_memo_entry(&shared_key)
                .is_none(),
            "after a catalog-generation bump no memo entry is current, so none \
             may be served as a stale answer"
        );
    });
}
