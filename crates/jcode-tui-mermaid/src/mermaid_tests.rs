include!("mermaid_tests/part_01.rs");
include!("mermaid_tests/part_02.rs");

mod ambient_root_tests {
    /// Regression: `MermaidCache::new()` resolved `dirs::cache_dir()` directly,
    /// so constructing a cache under test wrote rendered diagrams into the
    /// developer's real cache directory. Asserting "not under the real home"
    /// rather than a fixed path, because the harness redirect target is a
    /// per-process random temp dir.
    #[test]
    fn mermaid_cache_dir_is_not_under_the_real_home() {
        let cache = crate::MermaidCache::new();
        jcode_storage::assert_redirected_away_from_real_home(&cache.cache_dir, "mermaid cache dir");
    }
}
