use super::{
    AcpConfig, AmbientConfig, Config, ConfigProvenance, DiffDisplayMode, DisplayConfig,
    LatexRenderingMode, ProviderConfig, SessionPickerResumeAction, SwarmSpawnMode, ToolConfig,
    WarnOnce, config_env_fingerprint, populate_context_limits_from_config_ref,
};
use std::ffi::OsString;
use std::path::Path;

fn restore_env_var(key: &str, previous: Option<OsString>) {
    if let Some(previous) = previous {
        crate::env::set_var(key, previous);
    } else {
        crate::env::remove_var(key);
    }
}

#[test]
fn test_openai_reasoning_effort_defaults_to_low() {
    assert_eq!(
        ProviderConfig::default().openai_reasoning_effort.as_deref(),
        Some("low")
    );
}

#[test]
fn test_openai_fast_mode_defaults_to_priority() {
    assert_eq!(
        ProviderConfig::default().openai_service_tier.as_deref(),
        Some("priority")
    );
}

#[test]
fn preserve_reasoning_context_defaults_to_enabled() {
    assert!(ProviderConfig::default().preserve_reasoning_context);
}

#[test]
fn swarm_spawn_mode_defaults_to_inline() {
    assert_eq!(
        Config::default().agents.swarm_spawn_mode,
        SwarmSpawnMode::Inline
    );
}

#[test]
fn swarm_max_concurrent_agents_defaults_high_for_deep_fanout() {
    // Deep mode is meant to fan out wide; the default must be high (not the old
    // hardcoded run_plan default of 3).
    assert_eq!(Config::default().agents.swarm_max_concurrent_agents, 32);
}

#[test]
fn mermaid_feature_defaults_on_and_parses_false() {
    assert!(Config::default().features.mermaid);

    let cfg: Config =
        toml::from_str("[features]\nmermaid = false\n").expect("features.mermaid should parse");
    assert!(!cfg.features.mermaid);
}

#[test]
fn mermaid_environment_override_uses_standard_boolean_values() {
    let _guard = crate::storage::lock_test_env();
    let previous = std::env::var_os("JCODE_ENABLE_MERMAID");
    crate::env::set_var("JCODE_ENABLE_MERMAID", "off");

    let mut cfg = Config::default();
    cfg.apply_env_overrides();
    assert!(!cfg.features.mermaid);

    restore_env_var("JCODE_ENABLE_MERMAID", previous);
}

#[test]
fn latex_rendering_defaults_to_image_and_parses_all_modes() {
    assert_eq!(
        Config::default().display.latex_rendering,
        LatexRenderingMode::Image
    );
    for (value, expected) in [
        ("none", LatexRenderingMode::None),
        ("unicode", LatexRenderingMode::Unicode),
        ("image", LatexRenderingMode::Image),
    ] {
        let cfg: Config = toml::from_str(&format!("[display]\nlatex_rendering = \"{value}\"\n"))
            .expect("latex rendering mode should parse");
        assert_eq!(cfg.display.latex_rendering, expected);
        assert_eq!(LatexRenderingMode::parse(expected.as_str()), Some(expected));
    }
    assert!(toml::from_str::<Config>("[display]\nlatex_rendering = \"canvas\"\n").is_err());
}

#[test]
fn latex_rendering_environment_override_accepts_aliases() {
    let _guard = crate::storage::lock_test_env();
    let previous = std::env::var_os("JCODE_LATEX_RENDERING");
    crate::env::set_var("JCODE_LATEX_RENDERING", "png");
    let mut cfg = Config::default();
    cfg.apply_env_overrides();
    assert_eq!(cfg.display.latex_rendering, LatexRenderingMode::Image);
    restore_env_var("JCODE_LATEX_RENDERING", previous);
}

#[test]
fn swarm_max_concurrent_agents_parses_and_allows_zero_for_unbounded() {
    let cfg: Config = toml::from_str("[agents]\nswarm_max_concurrent_agents = 64\n")
        .expect("swarm_max_concurrent_agents should parse");
    assert_eq!(cfg.agents.swarm_max_concurrent_agents, 64);

    let cfg: Config = toml::from_str("[agents]\nswarm_max_concurrent_agents = 0\n")
        .expect("zero should parse (means unbounded up to the member cap)");
    assert_eq!(cfg.agents.swarm_max_concurrent_agents, 0);
}

#[test]
fn swarm_spawn_mode_parses_supported_values() {
    let cfg: Config = toml::from_str("[agents]\nswarm_spawn_mode = \"headless\"\n")
        .expect("headless swarm_spawn_mode should parse");
    assert_eq!(cfg.agents.swarm_spawn_mode, SwarmSpawnMode::Headless);

    let cfg: Config = toml::from_str("[agents]\nswarm_spawn_mode = \"auto\"\n")
        .expect("auto swarm_spawn_mode should parse");
    assert_eq!(cfg.agents.swarm_spawn_mode, SwarmSpawnMode::Auto);

    let cfg: Config = toml::from_str("[agents]\nswarm_spawn_mode = \"visible\"\n")
        .expect("visible swarm_spawn_mode should parse");
    assert_eq!(cfg.agents.swarm_spawn_mode, SwarmSpawnMode::Visible);
}

#[test]
fn swarm_spawn_mode_rejects_invalid_values() {
    let result = toml::from_str::<Config>("[agents]\nswarm_spawn_mode = \"background\"\n");
    assert!(result.is_err());
}

#[test]
fn swarm_spawn_mode_as_str_round_trips() {
    for mode in [
        SwarmSpawnMode::Visible,
        SwarmSpawnMode::Headless,
        SwarmSpawnMode::Auto,
    ] {
        assert_eq!(SwarmSpawnMode::parse(mode.as_str()), Some(mode));
    }
}

#[test]
fn test_env_override_swarm_spawn_mode() {
    let _guard = crate::storage::lock_test_env();
    let prev = std::env::var_os("JCODE_SWARM_SPAWN_MODE");
    crate::env::set_var("JCODE_SWARM_SPAWN_MODE", "headless");

    let mut cfg = Config::default();
    cfg.apply_env_overrides();

    assert_eq!(cfg.agents.swarm_spawn_mode, SwarmSpawnMode::Headless);

    restore_env_var("JCODE_SWARM_SPAWN_MODE", prev);
}

#[test]
fn test_env_override_swarm_model() {
    let _guard = crate::storage::lock_test_env();
    let prev = std::env::var_os("JCODE_SWARM_MODEL");
    crate::env::set_var("JCODE_SWARM_MODEL", "claude-opus-4-6");

    let mut cfg = Config::default();
    cfg.apply_env_overrides();

    assert_eq!(cfg.agents.swarm_model.as_deref(), Some("claude-opus-4-6"));

    // Empty value clears the override back to "inherit".
    crate::env::set_var("JCODE_SWARM_MODEL", "  ");
    let mut cfg = Config::default();
    cfg.agents.swarm_model = Some("preset".to_string());
    cfg.apply_env_overrides();
    assert_eq!(cfg.agents.swarm_model, None);

    restore_env_var("JCODE_SWARM_MODEL", prev);
}

#[test]
fn spawn_hook_defaults_to_none_and_parses_from_toml() {
    assert_eq!(Config::default().terminal.spawn_hook, None);

    let cfg: Config = toml::from_str("[terminal]\nspawn_hook = \"tmux new-window\"\n")
        .expect("spawn_hook should parse");
    assert_eq!(cfg.terminal.spawn_hook.as_deref(), Some("tmux new-window"));
}

#[test]
fn terminal_preferred_defaults_to_none_and_parses_from_toml() {
    assert_eq!(Config::default().terminal.preferred, None);

    let cfg: Config =
        toml::from_str("[terminal]\npreferred = \"ghostty\"\n").expect("preferred should parse");
    assert_eq!(cfg.terminal.preferred.as_deref(), Some("ghostty"));
}

#[test]
fn hooks_config_defaults_and_parses_from_toml() {
    let defaults = Config::default().hooks;
    assert_eq!(defaults.turn_start, None);
    assert_eq!(defaults.turn_end, None);
    assert_eq!(defaults.session_start, None);
    assert_eq!(defaults.session_end, None);
    assert_eq!(defaults.pre_tool, None);
    assert_eq!(defaults.post_tool, None);
    assert_eq!(defaults.pre_tool_timeout_ms, 5000);

    let cfg: Config = toml::from_str(
        "[hooks]\nturn_start = \"notify-start\"\nturn_end = \"notify-turn\"\npre_tool = \"~/bin/policy\"\npre_tool_timeout_ms = 1500\n",
    )
    .expect("hooks config should parse");
    assert_eq!(cfg.hooks.turn_start.as_deref(), Some("notify-start"));
    assert_eq!(cfg.hooks.turn_end.as_deref(), Some("notify-turn"));
    assert_eq!(cfg.hooks.pre_tool.as_deref(), Some("~/bin/policy"));
    assert_eq!(cfg.hooks.pre_tool_timeout_ms, 1500);
}

#[test]
fn test_env_override_lifecycle_hooks() {
    let _guard = crate::storage::lock_test_env();
    let prev_turn_end = std::env::var_os("JCODE_HOOK_TURN_END");
    let prev_timeout = std::env::var_os("JCODE_HOOK_PRE_TOOL_TIMEOUT_MS");

    crate::env::set_var("JCODE_HOOK_TURN_END", "my-notifier --fast");
    crate::env::set_var("JCODE_HOOK_PRE_TOOL_TIMEOUT_MS", "250");
    let mut cfg = Config::default();
    cfg.apply_env_overrides();
    assert_eq!(cfg.hooks.turn_end.as_deref(), Some("my-notifier --fast"));
    assert_eq!(cfg.hooks.pre_tool_timeout_ms, 250);

    // Empty env value disables a config-file hook.
    crate::env::set_var("JCODE_HOOK_TURN_END", " ");
    let mut cfg = Config::default();
    cfg.hooks.turn_end = Some("from-config".to_string());
    cfg.apply_env_overrides();
    assert_eq!(cfg.hooks.turn_end, None);

    restore_env_var("JCODE_HOOK_TURN_END", prev_turn_end);
    restore_env_var("JCODE_HOOK_PRE_TOOL_TIMEOUT_MS", prev_timeout);
}

#[test]
fn test_env_override_spawn_hook() {
    let _guard = crate::storage::lock_test_env();
    let prev = std::env::var_os("JCODE_SPAWN_HOOK");
    crate::env::set_var("JCODE_SPAWN_HOOK", "kitty @ launch --type=tab --");

    let mut cfg = Config::default();
    cfg.apply_env_overrides();
    assert_eq!(
        cfg.terminal.spawn_hook.as_deref(),
        Some("kitty @ launch --type=tab --")
    );

    // Empty env value disables a config-file hook.
    crate::env::set_var("JCODE_SPAWN_HOOK", "  ");
    let mut cfg = Config::default();
    cfg.terminal.spawn_hook = Some("tmux new-window".to_string());
    cfg.apply_env_overrides();
    assert_eq!(cfg.terminal.spawn_hook, None);

    restore_env_var("JCODE_SPAWN_HOOK", prev);
}

#[test]
fn test_env_override_focus_hook() {
    let _guard = crate::storage::lock_test_env();
    let prev = std::env::var_os("JCODE_FOCUS_HOOK");
    crate::env::set_var("JCODE_FOCUS_HOOK", "niri-focus-jcode");

    let mut cfg = Config::default();
    cfg.apply_env_overrides();
    assert_eq!(cfg.terminal.focus_hook.as_deref(), Some("niri-focus-jcode"));

    // Empty env value disables a config-file hook.
    crate::env::set_var("JCODE_FOCUS_HOOK", "");
    let mut cfg = Config::default();
    cfg.terminal.focus_hook = Some("wmctrl -a".to_string());
    cfg.apply_env_overrides();
    assert_eq!(cfg.terminal.focus_hook, None);

    restore_env_var("JCODE_FOCUS_HOOK", prev);
}

#[test]
fn test_memory_sidecar_enabled_defaults_true() {
    // The LLM precision-judge path is the only reliably productive memory mode,
    // so memory uses it by default. Users opt into the no-LLM hybrid path
    // explicitly by setting this false.
    let cfg = Config::default();
    assert!(cfg.agents.memory_sidecar_enabled);
}

#[test]
fn test_env_override_memory_sidecar() {
    let _guard = crate::storage::lock_test_env();
    let prev_model = std::env::var_os("JCODE_MEMORY_MODEL");
    let prev_enabled = std::env::var_os("JCODE_MEMORY_SIDECAR_ENABLED");
    crate::env::set_var("JCODE_MEMORY_MODEL", "claude-haiku-4");
    crate::env::set_var("JCODE_MEMORY_SIDECAR_ENABLED", "true");

    let mut cfg = Config::default();
    cfg.apply_env_overrides();

    assert_eq!(cfg.agents.memory_model.as_deref(), Some("claude-haiku-4"));
    assert!(cfg.agents.memory_sidecar_enabled);

    restore_env_var("JCODE_MEMORY_MODEL", prev_model);
    restore_env_var("JCODE_MEMORY_SIDECAR_ENABLED", prev_enabled);
}

#[test]
fn tool_config_defaults_to_full_toolset() {
    let selection = ToolConfig::default().selection();
    assert!(selection.allowed_tools.is_none());
    assert!(selection.disabled_tools.is_empty());
}

#[test]
fn tool_config_explicit_enabled_uses_allow_list() {
    let cfg = ToolConfig {
        enabled: vec!["gmail".to_string()],
        ..ToolConfig::default()
    };
    let selection = cfg.selection();
    let allowed = selection
        .allowed_tools
        .expect("explicit enabled is an allow-list");

    assert!(allowed.contains("gmail"));
    assert!(!selection.disabled_tools.contains("gmail"));
}

#[test]
fn tool_config_all_enabled_sentinel_keeps_unrestricted_toolset() {
    let cfg = ToolConfig {
        enabled: vec!["*".to_string()],
        ..ToolConfig::default()
    };
    let selection = cfg.selection();

    assert!(selection.allowed_tools.is_none());
    assert!(!selection.disabled_tools.contains("gmail"));
}

#[test]
fn tool_config_explicit_disabled_overrides_all_enabled_sentinel() {
    let cfg = ToolConfig {
        enabled: vec!["*".to_string()],
        disabled: vec!["gmail".to_string()],
        ..ToolConfig::default()
    };
    let selection = cfg.selection();

    assert!(selection.allowed_tools.is_none());
    assert!(selection.disabled_tools.contains("gmail"));
}

#[test]
fn tool_config_acp_profile_allows_core_coding_plus_batch() {
    let cfg = ToolConfig {
        profile: "acp".to_string(),
        ..ToolConfig::default()
    };
    let allowed = cfg.allowed_tools().expect("acp profile is an allow-list");

    assert!(allowed.contains("bash"));
    assert!(allowed.contains("read"));
    assert!(allowed.contains("write"));
    assert!(allowed.contains("apply_patch"));
    assert!(allowed.contains("agentgrep"));
    assert!(allowed.contains("batch"));
    assert!(!allowed.contains("swarm"));
    assert!(!allowed.contains("subagent"));
    assert!(!allowed.contains("side_panel"));
}

#[test]
fn acp_config_defaults_to_standard_profile_and_acp_tools() {
    let cfg = Config::default();
    assert_eq!(cfg.acp.profile, "standard");
    assert_eq!(cfg.acp.tool_profile, "acp");
}

#[test]
fn tool_config_minimal_profile_allows_core_coding_tools() {
    let cfg = ToolConfig {
        profile: "minimal".to_string(),
        ..ToolConfig::default()
    };
    let allowed = cfg
        .allowed_tools()
        .expect("minimal profile is an allow-list");

    assert!(allowed.contains("bash"));
    assert!(allowed.contains("read"));
    assert!(allowed.contains("write"));
    assert!(allowed.contains("apply_patch"));
    assert!(allowed.contains("agentgrep"));
    assert!(!allowed.contains("browser"));
    assert!(!allowed.contains("swarm"));
}

#[test]
fn tool_config_explicit_enabled_and_disabled_lists_compose() {
    let cfg = ToolConfig {
        enabled: vec![
            "shell".to_string(),
            "read_file".to_string(),
            "browser".to_string(),
        ],
        disabled: vec!["browser".to_string()],
        ..ToolConfig::default()
    };
    let selection = cfg.selection();
    let allowed = selection
        .allowed_tools
        .expect("explicit enabled is an allow-list");

    assert!(allowed.contains("bash"));
    assert!(allowed.contains("read"));
    assert!(!allowed.contains("shell"));
    assert!(!allowed.contains("read_file"));
    assert!(!allowed.contains("browser"));
    assert!(selection.disabled_tools.contains("browser"));
}

#[test]
fn tool_config_none_profile_disables_all_tools() {
    let cfg = ToolConfig {
        profile: "none".to_string(),
        ..ToolConfig::default()
    };
    assert!(
        cfg.allowed_tools()
            .expect("none profile is empty")
            .is_empty()
    );
}

#[test]
fn tool_config_disabled_only_keeps_full_profile_with_deny_list() {
    let cfg = ToolConfig {
        disabled: vec!["browser".to_string(), "swarm".to_string()],
        ..ToolConfig::default()
    };
    let selection = cfg.selection();

    assert!(selection.allowed_tools.is_none());
    assert!(selection.disabled_tools.contains("browser"));
    assert!(selection.disabled_tools.contains("swarm"));
    assert!(!selection.disabled_tools.contains("gmail"));
}

#[test]
fn test_generated_default_config_uses_low_openai_reasoning_effort() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let dir = tempfile::TempDir::new().expect("tempdir");
    crate::env::set_var("JCODE_HOME", dir.path());

    let path = Config::create_default_config_file().expect("create default config file");
    let content = std::fs::read_to_string(path).expect("read default config file");

    assert!(
        content.contains("openai_reasoning_effort = \"low\""),
        "generated default config should use low OpenAI reasoning effort"
    );
    assert!(
        content.contains("openai_service_tier = \"priority\""),
        "generated default config should enable OpenAI fast mode"
    );
    assert!(
        content.contains("[tools]") && content.contains("profile = \"full\""),
        "generated default config should document tool profiles"
    );
    assert!(
        content.contains("[acp]") && content.contains("tool_profile = \"acp\""),
        "generated default config should document ACP profile settings"
    );
    assert!(
        content.contains("[agents]") && content.contains("swarm_spawn_mode = \"inline\""),
        "generated default config should document agent spawn defaults"
    );
    assert!(
        content.contains("memory_model = \"gpt-5.6-luna\"")
            && content.contains("reasoning effort \"none\""),
        "generated default config should document the Luna memory sidecar default"
    );

    // Effort keys come from the per-platform keybinding registry; the template
    // placeholders must always be substituted.
    assert!(
        !content.contains("@EFFORT_INCREASE@") && !content.contains("@EFFORT_DECREASE@"),
        "generated default config should substitute effort key placeholders"
    );
    let expected_increase = if cfg!(target_os = "macos") {
        "effort_increase = \"cmd+right\""
    } else {
        "effort_increase = \"alt+right\""
    };
    assert!(
        content.contains(expected_increase),
        "generated default config should use the platform effort_increase default"
    );

    // The generated file must always be valid TOML for the current Config schema.
    let parsed: Config =
        toml::from_str(&content).expect("generated default config should parse as Config");
    assert_eq!(parsed.agents.swarm_spawn_mode, SwarmSpawnMode::Inline);

    if let Some(prev) = prev_home {
        crate::env::set_var("JCODE_HOME", prev);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
}

#[test]
fn global_config_cache_reloads_after_manual_file_edit() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let dir = tempfile::TempDir::new().expect("tempdir");
    crate::env::set_var("JCODE_HOME", dir.path());
    Config::invalidate_cache();

    let path = Config::path().expect("config path");
    std::fs::create_dir_all(path.parent().expect("config parent")).expect("create config parent");
    std::fs::write(&path, "[display]\ncentered = false\n").expect("write initial config");

    assert!(!crate::config::config().display.centered);

    // Different length as well as mtime so the metadata fingerprint notices the
    // manual edit even on filesystems with coarse timestamp resolution.
    std::fs::write(&path, "[display]\ncentered = true\n# edited\n").expect("edit config");

    assert!(crate::config::config().display.centered);

    restore_env_var("JCODE_HOME", prev_home);
    Config::invalidate_cache();
}

#[test]
fn config_save_invalidates_global_config_cache() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let dir = tempfile::TempDir::new().expect("tempdir");
    crate::env::set_var("JCODE_HOME", dir.path());
    Config::invalidate_cache();

    let mut cfg = Config::default();
    cfg.display.centered = false;
    cfg.save().expect("save initial config");
    assert!(!crate::config::config().display.centered);

    cfg.display.centered = true;
    cfg.save().expect("save updated config");
    assert!(crate::config::config().display.centered);

    restore_env_var("JCODE_HOME", prev_home);
    Config::invalidate_cache();
}

fn with_clean_config_env<T>(f: impl FnOnce() -> T) -> T {
    let keys = [
        "JCODE_MODEL",
        "JCODE_PROVIDER",
        "JCODE_OPENAI_REASONING_EFFORT",
        "JCODE_OPENAI_SERVICE_TIER",
        "JCODE_STREAM_IDLE_TIMEOUT_SECS",
        "JCODE_DISPLAY_CENTERED",
        "JCODE_WEBSEARCH_ENGINE",
    ];
    let previous = keys
        .iter()
        .map(|key| (*key, std::env::var_os(key)))
        .collect::<Vec<_>>();
    for key in keys {
        crate::env::remove_var(key);
    }
    let result = f();
    for (key, value) in previous {
        restore_env_var(key, value);
    }
    result
}

#[test]
fn config_load_merges_policy_over_durable() {
    let _guard = crate::storage::lock_test_env();
    with_clean_config_env(|| {
        let prev_home = std::env::var_os("JCODE_HOME");
        let dir = tempfile::TempDir::new().expect("tempdir");
        crate::env::set_var("JCODE_HOME", dir.path());
        Config::invalidate_cache();

        std::fs::write(
            dir.path().join("config.toml"),
            r#"
[provider]
default_model = "durable-model"
default_provider = "openai"
openai_service_tier = "flex"

[providers.gateway]
default_model = "durable-gateway-model"
"#,
        )
        .expect("write durable config");
        std::fs::write(
            dir.path().join("config.nix.toml"),
            r#"
[provider]
default_model = "policy-model"
stream_idle_timeout_secs = 600

[providers.gateway]
base_url = "https://policy.example/v1"
"#,
        )
        .expect("write policy config");

        let cfg = Config::load();
        assert_eq!(cfg.provider.default_model.as_deref(), Some("policy-model"));
        assert_eq!(cfg.provider.default_provider.as_deref(), Some("openai"));
        assert_eq!(cfg.provider.openai_service_tier.as_deref(), Some("flex"));
        assert_eq!(cfg.provider.stream_idle_timeout_secs, 600);
        let gateway = cfg
            .providers
            .get("gateway")
            .expect("merged gateway provider");
        assert_eq!(gateway.base_url, "https://policy.example/v1");
        assert_eq!(
            gateway.default_model.as_deref(),
            Some("durable-gateway-model")
        );

        restore_env_var("JCODE_HOME", prev_home);
        Config::invalidate_cache();
    });
}

#[test]
fn config_layer_provenance_marks_policy_durable_and_default_paths() {
    let _guard = crate::storage::lock_test_env();
    with_clean_config_env(|| {
        let prev_home = std::env::var_os("JCODE_HOME");
        let dir = tempfile::TempDir::new().expect("tempdir");
        crate::env::set_var("JCODE_HOME", dir.path());
        Config::invalidate_cache();

        std::fs::write(
            dir.path().join("config.toml"),
            "[provider]\ndefault_provider = \"openai\"\nopenai_service_tier = \"flex\"\n\n[providers.gateway]\ndefault_model = \"durable-gateway-model\"\n",
        )
        .expect("write durable config");
        std::fs::write(
            dir.path().join("config.nix.toml"),
            "[provider]\ndefault_model = \"policy-model\"\n\n[providers.gateway]\nbase_url = \"https://policy.example/v1\"\n",
        )
        .expect("write policy config");

        let cfg = Config::load();
        assert_eq!(
            cfg.provenance_for("provider.default_model"),
            ConfigProvenance::Policy
        );
        assert_eq!(
            cfg.provenance_for("provider.default_provider"),
            ConfigProvenance::Durable
        );
        assert_eq!(
            cfg.provenance_for("provider.openai_service_tier"),
            ConfigProvenance::Durable
        );
        assert_eq!(
            cfg.provenance_for("providers.gateway.base_url"),
            ConfigProvenance::Policy
        );
        assert_eq!(
            cfg.provenance_for("providers.gateway.default_model"),
            ConfigProvenance::Durable
        );
        assert_eq!(
            cfg.provenance_for("display.centered"),
            ConfigProvenance::Default
        );
        assert!(cfg.policy_pinned_paths().contains("provider.default_model"));

        restore_env_var("JCODE_HOME", prev_home);
        Config::invalidate_cache();
    });
}

#[test]
fn config_save_skips_policy_pinned_and_default_values() {
    let _guard = crate::storage::lock_test_env();
    with_clean_config_env(|| {
        let prev_home = std::env::var_os("JCODE_HOME");
        let dir = tempfile::TempDir::new().expect("tempdir");
        crate::env::set_var("JCODE_HOME", dir.path());
        Config::invalidate_cache();

        std::fs::write(
            dir.path().join("config.nix.toml"),
            "[provider]\ndefault_model = \"policy-model\"\nopenai_reasoning_effort = \"high\"\n",
        )
        .expect("write policy config");

        let mut cfg = Config::load();
        cfg.provider.default_model = Some("runtime-model".to_string());
        cfg.provider.default_provider = Some("copilot".to_string());
        cfg.provider.openai_reasoning_effort = Some("low".to_string());
        cfg.save().expect("save policy-aware config");

        let durable =
            std::fs::read_to_string(dir.path().join("config.toml")).expect("read durable config");
        assert!(durable.contains("default_provider = \"copilot\""));
        assert!(!durable.contains("default_model"));
        assert!(!durable.contains("openai_reasoning_effort"));
        assert!(!durable.contains("openai_service_tier"));

        let reloaded = Config::load();
        assert_eq!(
            reloaded.provider.default_model.as_deref(),
            Some("policy-model")
        );
        assert_eq!(
            reloaded.provider.default_provider.as_deref(),
            Some("copilot")
        );

        restore_env_var("JCODE_HOME", prev_home);
        Config::invalidate_cache();
    });
}

#[test]
fn pinned_config_save_warning_is_once_per_key() {
    Config::reset_pinned_config_save_warnings_for_tests();
    assert!(Config::warn_once_pinned_config_save(
        "provider.default_model"
    ));
    assert!(!Config::warn_once_pinned_config_save(
        "provider.default_model"
    ));
    assert!(Config::warn_once_pinned_config_save(
        "provider.default_provider"
    ));
}

#[test]
fn config_save_without_policy_is_bit_for_bit_legacy_pretty_toml() {
    let _guard = crate::storage::lock_test_env();
    with_clean_config_env(|| {
        let prev_home = std::env::var_os("JCODE_HOME");
        let dir = tempfile::TempDir::new().expect("tempdir");
        crate::env::set_var("JCODE_HOME", dir.path());
        Config::invalidate_cache();

        let mut cfg = Config::default();
        cfg.provider.default_model = Some("durable-model".to_string());
        cfg.provider.default_provider = Some("openai".to_string());
        cfg.display.centered = true;
        let expected = toml::to_string_pretty(&cfg).expect("legacy pretty toml");

        cfg.save().expect("save config without policy");
        let durable =
            std::fs::read_to_string(dir.path().join("config.toml")).expect("read durable config");
        assert_eq!(durable, expected);

        restore_env_var("JCODE_HOME", prev_home);
        Config::invalidate_cache();
    });
}

#[test]
fn locked_config_patch_preserves_two_concurrent_writers() {
    let _guard = crate::storage::lock_test_env();
    with_clean_config_env(|| {
        let prev_home = std::env::var_os("JCODE_HOME");
        let dir = tempfile::TempDir::new().expect("tempdir");
        crate::env::set_var("JCODE_HOME", dir.path());
        Config::invalidate_cache();

        let first_auth = dir.path().join("first-auth.json");
        let second_auth = dir.path().join("second-auth.json");
        std::fs::write(&first_auth, "{}\n").expect("write first auth file");
        std::fs::write(&second_auth, "{}\n").expect("write second auth file");

        let first = first_auth.clone();
        let second = second_auth.clone();
        let first_thread = std::thread::spawn(move || {
            Config::allow_external_auth_source_for_path("first", &first)
                .expect("first writer saves trust")
        });
        let second_thread = std::thread::spawn(move || {
            Config::allow_external_auth_source_for_path("second", &second)
                .expect("second writer saves trust")
        });

        first_thread.join().expect("first writer joins");
        second_thread.join().expect("second writer joins");

        let cfg = Config::load();
        assert!(Config::external_auth_source_allowed_for_path(
            "first",
            &first_auth
        ));
        assert!(Config::external_auth_source_allowed_for_path(
            "second",
            &second_auth
        ));
        assert_eq!(cfg.auth.trusted_external_source_paths.len(), 2);

        restore_env_var("JCODE_HOME", prev_home);
        Config::invalidate_cache();
    });
}

#[test]
fn config_env_fingerprint_ignores_runtime_only_jcode_vars() {
    let _guard = crate::storage::lock_test_env();
    let prev_runtime_provider = std::env::var_os("JCODE_RUNTIME_PROVIDER");
    let prev_active_provider = std::env::var_os("JCODE_ACTIVE_PROVIDER");
    let prev_display_centered = std::env::var_os("JCODE_DISPLAY_CENTERED");

    crate::env::remove_var("JCODE_RUNTIME_PROVIDER");
    crate::env::remove_var("JCODE_ACTIVE_PROVIDER");
    crate::env::remove_var("JCODE_DISPLAY_CENTERED");
    let baseline = config_env_fingerprint();

    crate::env::set_var("JCODE_RUNTIME_PROVIDER", "openai");
    crate::env::set_var("JCODE_ACTIVE_PROVIDER", "openai");
    assert_eq!(baseline, config_env_fingerprint());

    crate::env::set_var("JCODE_DISPLAY_CENTERED", "1");
    assert_ne!(baseline, config_env_fingerprint());

    restore_env_var("JCODE_RUNTIME_PROVIDER", prev_runtime_provider);
    restore_env_var("JCODE_ACTIVE_PROVIDER", prev_active_provider);
    restore_env_var("JCODE_DISPLAY_CENTERED", prev_display_centered);
}

#[test]
fn config_env_fingerprint_tracks_every_apply_env_override_var() {
    let override_source = include_str!("config/env_overrides.rs");
    let mut missing = Vec::new();

    for line in override_source.lines() {
        let Some(start) = line.find("std::env::var(\"") else {
            continue;
        };
        let rest = &line[start + "std::env::var(\"".len()..];
        let Some(end) = rest.find('"') else {
            continue;
        };
        let key = &rest[..end];
        if !crate::config::CONFIG_ENV_KEYS.contains(&key) {
            missing.push(key.to_string());
        }
    }

    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "CONFIG_ENV_KEYS must include every env var read by Config::apply_env_overrides; missing: {missing:?}"
    );
}

#[test]
fn cached_external_auth_trust_observes_manual_revocation() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let dir = tempfile::TempDir::new().expect("tempdir");
    crate::env::set_var("JCODE_HOME", dir.path());
    Config::invalidate_cache();

    let auth_file = dir.path().join("external-auth.json");
    std::fs::write(&auth_file, "{}\n").expect("write external auth file");
    Config::allow_external_auth_source_for_path("test_source", &auth_file)
        .expect("trust external auth path");
    assert!(Config::external_auth_source_allowed_for_path_cached(
        "test_source",
        &auth_file
    ));

    let path = Config::path().expect("config path");
    std::fs::write(
        &path,
        "[auth]\ntrusted_external_source_paths = []\n# manually revoked\n",
    )
    .expect("manually revoke external auth trust");

    assert!(!Config::external_auth_source_allowed_for_path_cached(
        "test_source",
        &auth_file
    ));

    restore_env_var("JCODE_HOME", prev_home);
    Config::invalidate_cache();
}

#[test]
fn test_ambient_visible_defaults_to_true() {
    assert!(AmbientConfig::default().visible);
}

#[test]
fn test_display_auto_server_reload_defaults_to_true() {
    assert!(DisplayConfig::default().auto_server_reload);
}

#[test]
fn test_display_alignment_defaults_to_left() {
    assert!(!DisplayConfig::default().centered);
}

#[test]
fn test_provider_failover_defaults_match_new_behavior() {
    let provider = Config::default().provider;
    assert_eq!(
        provider.cross_provider_failover,
        super::CrossProviderFailoverMode::Countdown
    );
    assert!(provider.same_provider_account_failover);
}

#[test]
fn test_native_scrollbars_default_to_enabled() {
    let display = DisplayConfig::default();
    assert!(display.native_scrollbars.chat);
    assert!(display.native_scrollbars.side_panel);
}

#[test]
fn test_copy_badge_alt_label_defaults_to_auto_and_deserializes() {
    assert!(DisplayConfig::default().copy_badge_alt_label.is_empty());

    let cfg: Config = toml::from_str(
        r#"
        [display]
        copy_badge_alt_label = "Option"
        "#,
    )
    .expect("config should deserialize");

    assert_eq!(cfg.display.copy_badge_alt_label, "Option");
}

#[test]
fn test_session_picker_resume_action_defaults_to_current_terminal() {
    assert_eq!(
        Config::default().keybindings.session_picker_enter,
        SessionPickerResumeAction::CurrentTerminal
    );
    assert_eq!(
        SessionPickerResumeAction::CurrentTerminal.alternate(),
        SessionPickerResumeAction::NewTerminal
    );
}

#[test]
fn test_session_picker_resume_action_deserializes_kebab_case() {
    let cfg: Config = toml::from_str(
        r#"
        [keybindings]
        session_picker_enter = "current-terminal"
        "#,
    )
    .expect("config should deserialize");

    assert_eq!(
        cfg.keybindings.session_picker_enter,
        SessionPickerResumeAction::CurrentTerminal
    );
}

#[test]
fn test_env_override_auto_server_reload() {
    let _guard = crate::storage::lock_test_env();
    let prev = std::env::var_os("JCODE_AUTO_SERVER_RELOAD");
    crate::env::set_var("JCODE_AUTO_SERVER_RELOAD", "false");

    let mut cfg = Config::default();
    cfg.apply_env_overrides();

    assert!(!cfg.display.auto_server_reload);

    if let Some(prev) = prev {
        crate::env::set_var("JCODE_AUTO_SERVER_RELOAD", prev);
    } else {
        crate::env::remove_var("JCODE_AUTO_SERVER_RELOAD");
    }
}

#[test]
fn test_env_override_native_scrollbars() {
    let _guard = crate::storage::lock_test_env();
    let prev_chat = std::env::var_os("JCODE_CHAT_NATIVE_SCROLLBAR");
    let prev_side = std::env::var_os("JCODE_SIDE_PANEL_NATIVE_SCROLLBAR");
    crate::env::set_var("JCODE_CHAT_NATIVE_SCROLLBAR", "true");
    crate::env::set_var("JCODE_SIDE_PANEL_NATIVE_SCROLLBAR", "false");

    let mut cfg = Config::default();
    cfg.apply_env_overrides();

    assert!(cfg.display.native_scrollbars.chat);
    assert!(!cfg.display.native_scrollbars.side_panel);

    if let Some(prev) = prev_chat {
        crate::env::set_var("JCODE_CHAT_NATIVE_SCROLLBAR", prev);
    } else {
        crate::env::remove_var("JCODE_CHAT_NATIVE_SCROLLBAR");
    }
    if let Some(prev) = prev_side {
        crate::env::set_var("JCODE_SIDE_PANEL_NATIVE_SCROLLBAR", prev);
    } else {
        crate::env::remove_var("JCODE_SIDE_PANEL_NATIVE_SCROLLBAR");
    }
}

#[test]
fn test_env_override_diff_mode_full_inline() {
    let _guard = crate::storage::lock_test_env();
    let prev = std::env::var_os("JCODE_DIFF_MODE");
    crate::env::set_var("JCODE_DIFF_MODE", "full-inline");

    let mut cfg = Config::default();
    cfg.apply_env_overrides();

    assert_eq!(cfg.display.diff_mode, DiffDisplayMode::FullInline);

    if let Some(prev) = prev {
        crate::env::set_var("JCODE_DIFF_MODE", prev);
    } else {
        crate::env::remove_var("JCODE_DIFF_MODE");
    }
}

#[test]
fn test_env_override_trusted_external_auth_splits_source_and_path_entries() {
    let _guard = crate::storage::lock_test_env();
    let prev = std::env::var_os("JCODE_TRUSTED_EXTERNAL_AUTH_SOURCES");
    crate::env::set_var(
        "JCODE_TRUSTED_EXTERNAL_AUTH_SOURCES",
        "legacy_source,claude_code_credentials|/tmp/auth.json",
    );

    let mut cfg = Config::default();
    cfg.apply_env_overrides();

    assert_eq!(cfg.auth.trusted_external_sources, vec!["legacy_source"]);
    assert_eq!(
        cfg.auth.trusted_external_source_paths,
        vec!["claude_code_credentials|/tmp/auth.json"]
    );

    if let Some(prev) = prev {
        crate::env::set_var("JCODE_TRUSTED_EXTERNAL_AUTH_SOURCES", prev);
    } else {
        crate::env::remove_var("JCODE_TRUSTED_EXTERNAL_AUTH_SOURCES");
    }
}

#[test]
fn test_external_auth_source_allowed_for_path_matches_saved_entry() {
    let _guard = crate::storage::lock_test_env();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("auth.json");
    std::fs::write(&path, "{}\n").expect("write auth file");

    let canonical = std::fs::canonicalize(&path).expect("canonical path");
    let mut cfg = Config::default();
    cfg.auth.trusted_external_source_paths = vec![format!(
        "test_source|{}",
        canonical.to_string_lossy().to_ascii_lowercase()
    )];

    assert!(cfg.external_auth_source_allowed_for_path_config("test_source", &path));
}

#[test]
fn test_external_auth_source_allowed_for_path_ignores_broad_legacy_entry() {
    let _guard = crate::storage::lock_test_env();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("auth.json");
    std::fs::write(&path, "{}\n").expect("write auth file");

    let mut cfg = Config::default();
    cfg.auth.trusted_external_sources = vec!["test_source".to_string()];

    assert!(!cfg.external_auth_source_allowed_for_path_config("test_source", &path));
}

/// Regression test for issue #349: a removed/unknown `update_channel` value
/// (older configs could contain `"manual"`) must not fail the whole config
/// parse. A hard parse failure during the reload handoff left the reload
/// marker stuck in `starting` and clients re-requested the reload forever.
#[test]
fn unknown_update_channel_value_falls_back_to_stable_instead_of_failing_parse() {
    let cfg: Config = toml::from_str("[features]\nupdate_channel = \"manual\"\n")
        .expect("unknown update_channel must not fail config parse");
    assert_eq!(
        cfg.features.update_channel,
        super::UpdateChannel::Stable,
        "unknown channel should fall back to the default"
    );

    // Other settings in the same config must survive the fallback.
    let cfg: Config = toml::from_str(
        "[features]\nupdate_channel = \"manual\"\nmemory = false\n\n[display]\ncentered = true\n",
    )
    .expect("config with unknown update_channel should parse");
    assert_eq!(cfg.features.update_channel, super::UpdateChannel::Stable);
    assert!(!cfg.features.memory);
    assert!(cfg.display.centered);
}

#[test]
fn known_update_channel_values_still_parse() {
    let cfg: Config = toml::from_str("[features]\nupdate_channel = \"main\"\n")
        .expect("main update_channel should parse");
    assert_eq!(cfg.features.update_channel, super::UpdateChannel::Main);

    let cfg: Config = toml::from_str("[features]\nupdate_channel = \"stable\"\n")
        .expect("stable update_channel should parse");
    assert_eq!(cfg.features.update_channel, super::UpdateChannel::Stable);
}

#[test]
fn update_channel_parse_accepts_known_aliases_and_rejects_unknown() {
    use super::UpdateChannel;
    assert_eq!(UpdateChannel::parse("stable"), Some(UpdateChannel::Stable));
    assert_eq!(UpdateChannel::parse("release"), Some(UpdateChannel::Stable));
    assert_eq!(UpdateChannel::parse("main"), Some(UpdateChannel::Main));
    assert_eq!(UpdateChannel::parse("nightly"), Some(UpdateChannel::Main));
    assert_eq!(UpdateChannel::parse("edge"), Some(UpdateChannel::Main));
    assert_eq!(UpdateChannel::parse(" Main "), Some(UpdateChannel::Main));
    assert_eq!(UpdateChannel::parse("manual"), None);
    assert_eq!(UpdateChannel::parse(""), None);
}

impl Config {
    fn external_auth_source_allowed_for_path_config(&self, source_id: &str, path: &Path) -> bool {
        let Ok(entry) = Self::trusted_external_auth_path_entry(source_id, path) else {
            return false;
        };
        self.auth
            .trusted_external_source_paths
            .iter()
            .any(|value| value.trim().eq_ignore_ascii_case(&entry))
    }
}

#[test]
fn populate_context_limits_from_config_ref_seeds_global_cache() {
    use super::{NamedProviderConfig, NamedProviderModelConfig};

    // Regression test for issue #366: a named OpenAI-compatible provider with a
    // per-model `context_window` must be honored by the global context-limit
    // resolution path, not just the provider instance's own context_window().
    let model_id = "issue366-custom-gateway-model";
    let mut cfg = Config::default();
    cfg.providers.insert(
        "issue366-gateway".to_string(),
        NamedProviderConfig {
            base_url: "https://gateway.example.test/v1".to_string(),
            models: vec![NamedProviderModelConfig {
                id: model_id.to_string(),
                context_window: Some(1_000_000),
                input: Vec::new(),
            }],
            ..Default::default()
        },
    );

    populate_context_limits_from_config_ref(&cfg);

    assert_eq!(
        crate::provider::context_limit_for_model(model_id),
        Some(1_000_000),
        "global context-limit resolution should respect named provider context_window"
    );
}

#[test]
fn populate_context_limits_from_config_seeds_qualified_runtime_model_shapes() {
    use super::{NamedProviderConfig, NamedProviderModelConfig};

    // Regression test for issue #421: the runtime request model can be
    // provider-qualified (`cachyai-a2000:qwen...`) or a slash path served by
    // llama.cpp (`ornith-box-1:/opt/models/ornith-1.0-35b-Q4_K_M.gguf`). The
    // configured context_window must resolve for every shape, not just the
    // bare id, otherwise budgeting falls back to the 200K default and
    // over-sends context.
    let mut cfg = Config::default();
    cfg.providers.insert(
        "issue421-gateway".to_string(),
        NamedProviderConfig {
            base_url: "http://10.15.15.53:8080/v1".to_string(),
            models: vec![
                NamedProviderModelConfig {
                    id: "issue421-qwen-128k".to_string(),
                    context_window: Some(131_072),
                    input: Vec::new(),
                },
                NamedProviderModelConfig {
                    id: "/opt/models/issue421-ornith-35b-q4.gguf".to_string(),
                    context_window: Some(131_072),
                    input: Vec::new(),
                },
            ],
            ..Default::default()
        },
    );

    populate_context_limits_from_config_ref(&cfg);

    // Bare id.
    assert_eq!(
        crate::provider::context_limit_for_model("issue421-qwen-128k"),
        Some(131_072)
    );
    // Profile-qualified spec, as persisted by session restore.
    assert_eq!(
        crate::provider::context_limit_for_model("issue421-gateway:issue421-qwen-128k"),
        Some(131_072),
        "profile-qualified model spec must resolve the configured context_window"
    );
    // Slash-path model id: the lookup reduces to the slash base.
    assert_eq!(
        crate::provider::context_limit_for_model("/opt/models/issue421-ornith-35b-q4.gguf"),
        Some(131_072),
        "slash-path model id must resolve the configured context_window"
    );
    // Profile-qualified slash-path spec, exactly as reported in issue #421.
    assert_eq!(
        crate::provider::context_limit_for_model(
            "issue421-gateway:/opt/models/issue421-ornith-35b-q4.gguf"
        ),
        Some(131_072),
        "profile-qualified slash-path spec must resolve the configured context_window"
    );
}

// ---------------------------------------------------------------------------
// WI-4: observable config parsing (unknown keys), the one load-time
// normalization (memory_embedding_backend), and warn-once semantics.
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
    let mut cfg = AcpConfig::default();
    cfg.profile = " extended ".to_string();
    assert_eq!(cfg.normalized_profile(), "extended");
    cfg.profile = "bogus-wi4-acp".to_string();
    assert_eq!(cfg.normalized_profile(), "standard");
}
