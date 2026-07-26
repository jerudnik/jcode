//! Tests for the assistant profile/config types.

use super::*;

#[test]
fn profile_parses_full_toml() {
    let toml = r#"
[profiles.infra]
cwd = "/home/john/infrastructure/4nix"
display_name = "Infra"
session_name_pattern = "assistant-{profile}"
model = "claude-opus-4-6"
provider = "claude"
memory_scope = "project"
startup_reminder = "Stay in 4nix."
zmx_session = "jcode-assistant-infra"
"#;
    let config: AssistantProfilesConfig = toml::from_str(toml).expect("parse");
    let profile = config.get("infra").expect("infra profile");
    assert_eq!(profile.cwd, "/home/john/infrastructure/4nix");
    assert_eq!(profile.display_name_or("infra"), "Infra");
    assert_eq!(profile.session_name("infra"), "assistant-infra");
    assert_eq!(profile.model.as_deref(), Some("claude-opus-4-6"));
    assert_eq!(profile.provider.as_deref(), Some("claude"));
    assert_eq!(profile.memory_scope, AssistantMemoryScope::Project);
    assert_eq!(profile.startup_reminder.as_deref(), Some("Stay in 4nix."));
    assert_eq!(
        profile.zmx_session.as_deref(),
        Some("jcode-assistant-infra")
    );
    // Mode defaults to execute when unspecified.
    assert_eq!(profile.mode, AssistantMode::Execute);
    config.validate().expect("valid");
}

#[test]
fn profile_parses_converse_mode() {
    let toml = r#"
[profiles.jcode]
cwd = "/home/john/infrastructure/jcode"
mode = "converse"
startup_reminder = "You know the self-dev loop."
"#;
    let config: AssistantProfilesConfig = toml::from_str(toml).expect("parse");
    let profile = config.get("jcode").expect("jcode profile");
    assert_eq!(profile.mode, AssistantMode::Converse);
    assert_eq!(profile.mode.as_str(), "converse");
}

#[test]
fn resolved_persona_combines_mode_and_reminder() {
    // execute + reminder => just the reminder.
    let execute = AssistantProfile {
        cwd: "/tmp".to_string(),
        startup_reminder: Some("  Stay in 4nix.  ".to_string()),
        ..AssistantProfile::default()
    };
    assert_eq!(execute.resolved_persona().as_deref(), Some("Stay in 4nix."));

    // execute + no reminder => nothing injected.
    let bare = AssistantProfile {
        cwd: "/tmp".to_string(),
        ..AssistantProfile::default()
    };
    assert!(bare.resolved_persona().is_none());

    // converse + no reminder => stance preamble only.
    let converse = AssistantProfile {
        cwd: "/tmp".to_string(),
        mode: AssistantMode::Converse,
        ..AssistantProfile::default()
    };
    let persona = converse.resolved_persona().expect("converse persona");
    assert!(persona.contains("collaborative"));

    // converse + reminder => stance preamble then reminder.
    let both = AssistantProfile {
        cwd: "/tmp".to_string(),
        mode: AssistantMode::Converse,
        startup_reminder: Some("Stay in 4nix.".to_string()),
        ..AssistantProfile::default()
    };
    let persona = both.resolved_persona().expect("combined persona");
    let stance_at = persona.find("collaborative").expect("stance present");
    let reminder_at = persona.find("Stay in 4nix.").expect("reminder present");
    assert!(stance_at < reminder_at, "stance precedes reminder");
}

#[test]
fn minimal_profile_uses_defaults() {
    let toml = r#"
[profiles.scratch]
cwd = "/tmp"
"#;
    let config: AssistantProfilesConfig = toml::from_str(toml).expect("parse");
    let profile = config.get("scratch").expect("scratch profile");
    assert_eq!(profile.display_name_or("scratch"), "scratch");
    assert_eq!(profile.session_name("scratch"), "assistant-scratch");
    assert_eq!(profile.memory_scope, AssistantMemoryScope::Project);
    assert!(profile.model.is_none());
    assert!(profile.zmx_session.is_none());
}

#[test]
fn session_name_pattern_expands_profile_token() {
    let profile = AssistantProfile {
        cwd: "/tmp".to_string(),
        session_name_pattern: Some("jcode-{profile}-shell".to_string()),
        ..AssistantProfile::default()
    };
    assert_eq!(profile.session_name("jcode"), "jcode-jcode-shell");
}

#[test]
fn missing_cwd_fails_validation() {
    let profile = AssistantProfile::default();
    assert_eq!(
        profile.validate("infra"),
        Err(AssistantProfileError::MissingCwd {
            profile: "infra".to_string()
        })
    );
}

#[test]
fn validate_reports_first_bad_profile() {
    let mut config = AssistantProfilesConfig::default();
    config.profiles.insert(
        "ok".to_string(),
        AssistantProfile {
            cwd: "/tmp".to_string(),
            ..AssistantProfile::default()
        },
    );
    config
        .profiles
        .insert("bad".to_string(), AssistantProfile::default());
    assert!(config.validate().is_err());
}

#[test]
fn expand_home_handles_tilde() {
    // `HOME` is process-global and feeds jcode-base's config cache
    // fingerprint. This crate sits below jcode-base and cannot reach its
    // test environment lease, so serialize against this crate's own tests
    // with a local static mutex and restore `HOME` before releasing it.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Save and set HOME deterministically.
    let prev = std::env::var("HOME").ok();
    unsafe {
        std::env::set_var("HOME", "/home/tester");
    }
    assert_eq!(expand_home("~"), "/home/tester");
    assert_eq!(expand_home("~/infra/4nix"), "/home/tester/infra/4nix");
    assert_eq!(expand_home("/abs/path"), "/abs/path");
    match prev {
        Some(value) => unsafe { std::env::set_var("HOME", value) },
        None => unsafe { std::env::remove_var("HOME") },
    }
}
