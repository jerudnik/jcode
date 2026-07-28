//! WI-4 config tests: observable parsing of unknown keys, the one load-time
//! normalization (`memory_embedding_backend`), and warn-once semantics.
//!
//! Split out of `config_tests.rs` to keep both files under the test-size budget.

use super::tests::restore_env_var;
use super::{AcpConfig, Config, WarnOnce};
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------

#[test]
fn unknown_top_level_and_nested_config_keys_are_collected() {
    let toml = "\
redraw_fpss = 30\n\
[display]\n\
redraw_fps = 45\n\
totally_unknown = true\n\
[provider]\n\
default_model = \"gpt-5.5\"\n";
    let (config, unknown) =
        Config::parse_toml_collecting_unknown(toml).expect("permissive parse should succeed");
    // Known keys still deserialize.
    assert_eq!(config.display.redraw_fps, 45);
    assert_eq!(config.provider.default_model.as_deref(), Some("gpt-5.5"));
    // Unknown keys are collected, sorted, and deduped.
    let collected: Vec<&str> = unknown.iter().map(String::as_str).collect();
    assert_eq!(collected, vec!["display.totally_unknown", "redraw_fpss"]);
}

#[test]
fn unknown_config_key_warnings_are_bounded_to_current_fingerprint() {
    let _guard = crate::storage::lock_test_env();
    Config::reset_unknown_config_key_warnings_for_tests();
    let first_keys = BTreeSet::from([
        "agents.retired_option".to_string(),
        "display.misspelled_option".to_string(),
    ]);
    let mut emitted = Vec::new();

    assert_eq!(
        Config::warn_unknown_config_keys_once_with(41, &first_keys, |key| {
            emitted.push(key.to_string())
        }),
        2
    );
    assert_eq!(
        Config::warn_unknown_config_keys_once_with(41, &first_keys, |key| {
            emitted.push(key.to_string())
        }),
        0
    );
    assert_eq!(
        emitted,
        vec![
            "agents.retired_option".to_string(),
            "display.misspelled_option".to_string(),
        ]
    );

    let changed_keys = BTreeSet::from(["provider.new_unknown".to_string()]);
    assert_eq!(
        Config::warn_unknown_config_keys_once_with(42, &changed_keys, |key| {
            emitted.push(key.to_string())
        }),
        1
    );
    assert_eq!(
        Config::unknown_config_key_warning_state_for_tests(),
        (Some(42), vec!["provider.new_unknown".to_string()]),
        "a fingerprint change must discard prior keys instead of growing process state"
    );
}

#[test]
fn fully_known_config_produces_no_unknown_keys() {
    let toml = "\
[display]\n\
redraw_fps = 45\n\
animation_fps = 30\n\
[provider]\n\
default_model = \"gpt-5.5\"\n";
    let (_config, unknown) =
        Config::parse_toml_collecting_unknown(toml).expect("known-only parse should succeed");
    assert!(
        unknown.is_empty(),
        "expected no unknown keys, got: {:?}",
        unknown
    );
}

#[test]
fn malformed_config_returns_parse_error_and_no_keys() {
    // Missing closing quote -> syntactically invalid TOML.
    let toml = "[provider]\ndefault_model = \"unterminated\n";
    let result = Config::parse_toml_collecting_unknown(toml);
    assert!(
        result.is_err(),
        "malformed TOML must return the underlying parse error"
    );
}

#[test]
fn memory_embedding_backend_normalizes_case_from_file() {
    for raw in ["OpenAI", "OPENAI", "  openai  "] {
        let toml = format!("[agents]\nmemory_embedding_backend = \"{raw}\"\n");
        let (mut config, _unknown) =
            Config::parse_toml_collecting_unknown(&toml).expect("parse should succeed");
        config.normalize_memory_embedding_backend();
        assert_eq!(
            config.agents.memory_embedding_backend, "openai",
            "'{raw}' should normalize to exact lowercase 'openai'"
        );
    }

    let toml = "[agents]\nmemory_embedding_backend = \"LOCAL\"\n";
    let (mut config, _unknown) =
        Config::parse_toml_collecting_unknown(toml).expect("parse should succeed");
    config.normalize_memory_embedding_backend();
    assert_eq!(config.agents.memory_embedding_backend, "local");
}

#[test]
fn memory_embedding_backend_garbage_falls_back_to_local() {
    let toml = "[agents]\nmemory_embedding_backend = \"garbage\"\n";
    let (mut config, _unknown) =
        Config::parse_toml_collecting_unknown(toml).expect("parse should succeed");
    config.normalize_memory_embedding_backend();
    assert_eq!(
        config.agents.memory_embedding_backend, "local",
        "unrecognized backend must fall back to 'local'"
    );
}

#[test]
fn memory_embedding_backend_normalizes_env_reintroduced_bad_value() {
    // Env override can reintroduce a bad value; the normalizer runs AFTER
    // apply_env_overrides so this must still land on a valid backend.
    //
    // `JCODE_MEMORY_EMBEDDING_BACKEND` is part of the global config cache
    // fingerprint, so mutating it without the environment lease races the
    // cache-generation assertions in tests such as
    // `global_config_cache_reloads_after_manual_file_edit`.
    let _guard = crate::storage::lock_test_env();
    let key = "JCODE_MEMORY_EMBEDDING_BACKEND";
    let previous = std::env::var_os(key);
    crate::env::set_var(key, "OpenAI");
    let mut config = Config::default();
    config.apply_env_overrides();
    // Sanity: env override applied the raw (mixed-case) value.
    assert_eq!(config.agents.memory_embedding_backend, "OpenAI");
    config.normalize_memory_embedding_backend();
    assert_eq!(config.agents.memory_embedding_backend, "openai");

    crate::env::set_var(key, "garbage");
    let mut config = Config::default();
    config.apply_env_overrides();
    config.normalize_memory_embedding_backend();
    assert_eq!(config.agents.memory_embedding_backend, "local");

    restore_env_var(key, previous);
}

#[test]
fn warn_once_fires_exactly_once_across_repeated_calls() {
    let guard = WarnOnce::new();
    assert!(guard.should_fire(), "first call must fire");
    for _ in 0..5 {
        assert!(!guard.should_fire(), "subsequent calls must not fire");
    }
}

#[test]
fn wi4_keyed_config_fallback_warning_only_fires_once_per_setting_raw_fallback() {
    let setting = "wi4.test.setting";
    let raw = "bogus-wi4-once";
    assert!(crate::config::warn_once_configured_string_fallback(
        setting,
        raw,
        "fallback-a",
        "fallback-a|known"
    ));
    assert!(!crate::config::warn_once_configured_string_fallback(
        setting,
        raw,
        "fallback-a",
        "fallback-a|known"
    ));
    assert!(crate::config::warn_once_configured_string_fallback(
        setting,
        raw,
        "fallback-b",
        "fallback-b|known"
    ));
}

#[test]
fn wi4_acp_profile_parser_preserves_aliases_and_fallback() {
    let mut cfg = AcpConfig {
        profile: " extended ".to_string(),
        ..AcpConfig::default()
    };
    assert_eq!(cfg.normalized_profile(), "extended");
    cfg.profile = "bogus-wi4-acp".to_string();
    assert_eq!(cfg.normalized_profile(), "standard");
}
