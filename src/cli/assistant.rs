//! Assistant-profile resolution and launch (`jcode assistant ...`).
//!
//! Assistant profiles turn Jcode into a persistent assistant shell for a named
//! context (e.g. `infra`, `jcode`, `scratch`). Profiles live in normal Jcode
//! config (`[assistant.profiles.<name>]`); 4nix may generate this config later
//! but is never required. Launching a profile resolves its working directory,
//! resumes or creates a deterministic session, persists assistant metadata, and
//! hands off to the standard TUI launch path.

use anyhow::Result;

use crate::config::{AssistantProfile, AssistantProfileError, Config};
use crate::session::{AssistantSessionMeta, Session};

use super::args::Args;
use super::output;

/// Resolve a profile by name from config, or produce a useful error with the
/// list of available profiles.
pub(crate) fn resolve_profile(
    config: &Config,
    name: &str,
) -> std::result::Result<AssistantProfile, AssistantProfileError> {
    match config.assistant.get(name) {
        Some(profile) => {
            profile.validate(name)?;
            Ok(profile.clone())
        }
        None => Err(AssistantProfileError::NotFound {
            requested: name.to_string(),
            available: config
                .assistant
                .names()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }),
    }
}

/// A short, deterministic example config block shown when a profile is missing
/// or no profiles are configured.
pub(crate) fn example_config(profile_hint: &str) -> String {
    let name = if profile_hint.trim().is_empty() {
        "infra"
    } else {
        profile_hint
    };
    format!(
        "Add a profile to ~/.jcode/config.toml, for example:\n\n\
         [assistant.profiles.{name}]\n\
         cwd = \"~/infrastructure/4nix\"\n\
         display_name = \"{title}\"\n\
         model = \"claude-opus-4-6\"\n\
         provider = \"claude\"\n\
         mode = \"converse\"\n\
         startup_reminder = \"You are the {name} assistant.\"\n",
        name = name,
        title = capitalize(name),
    )
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Deterministic session id for a profile so repeated launches resume the same
/// session. Format: `session_<shortname>_assistant_<profile>`. The short name
/// segment keeps `extract_session_name` working for window titles.
pub(crate) fn assistant_session_id(profile_name: &str, profile: &AssistantProfile) -> String {
    let short = profile.session_name(profile_name);
    let safe_short = sanitize_id_segment(&short);
    let safe_profile = sanitize_id_segment(profile_name);
    format!("session_{safe_short}_assistant_{safe_profile}")
}

/// Keep id segments to `[a-z0-9-]`, collapsing other characters to `-`.
fn sanitize_id_segment(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "profile".to_string()
    } else {
        trimmed
    }
}

/// Build the assistant metadata recorded on the session.
pub(crate) fn build_session_meta(
    profile_name: &str,
    profile: &AssistantProfile,
) -> AssistantSessionMeta {
    AssistantSessionMeta {
        profile: profile_name.to_string(),
        display_name: Some(profile.display_name_or(profile_name).to_string()),
        cwd: Some(profile.resolved_cwd()),
        backing: profile.zmx_session.clone(),
        last_checkpoint: None,
        last_validation: None,
        persona: profile.resolved_persona(),
    }
}

/// Ensure the assistant session exists on disk with up-to-date metadata.
///
/// Creates the session file if it does not exist, or refreshes assistant
/// metadata on an existing session. Returns the session id to resume.
pub(crate) fn ensure_session(profile_name: &str, profile: &AssistantProfile) -> Result<String> {
    let session_id = assistant_session_id(profile_name, profile);
    let meta = build_session_meta(profile_name, profile);
    let cwd = profile.resolved_cwd();

    let mut session = match Session::load(&session_id) {
        Ok(existing) => existing,
        Err(_) => {
            let mut created = Session::create_with_id(
                session_id.clone(),
                None,
                Some(profile.display_name_or(profile_name).to_string()),
            );
            created.short_name = Some(profile.session_name(profile_name));
            created
        }
    };

    session.working_dir = Some(cwd);
    session.assistant = Some(meta);
    if let Some(model) = profile.model.clone()
        && session.model.is_none()
    {
        session.model = Some(model);
    }
    session.save()?;
    Ok(session_id)
}

/// Render a single profile's status block as text.
fn render_status_text(profile_name: &str, profile: &AssistantProfile, session_id: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Assistant profile: {}\n",
        profile.display_name_or(profile_name)
    ));
    out.push_str(&format!("  key:     {profile_name}\n"));
    out.push_str(&format!("  cwd:     {}\n", profile.resolved_cwd()));
    out.push_str(&format!("  session: {}\n", session_id));
    if let Some(model) = profile.model.as_deref() {
        out.push_str(&format!("  model:   {model}\n"));
    }
    if let Some(provider) = profile.provider.as_deref() {
        out.push_str(&format!("  provider: {provider}\n"));
    }
    out.push_str(&format!("  memory:  {}\n", profile.memory_scope.as_str()));
    out.push_str(&format!("  mode:    {}\n", profile.mode.as_str()));
    if let Some(backing) = profile.zmx_session.as_deref() {
        out.push_str(&format!("  backing: zmx: {backing}\n"));
        out.push_str(&format!("    zmx attach {backing}\n"));
        out.push_str(&format!("    zmx history {backing}\n"));
    }
    let exists = crate::session::session_exists(session_id);
    out.push_str(&format!(
        "  state:   {}\n",
        if exists {
            "session on disk (will resume)"
        } else {
            "no session yet (will create on launch)"
        }
    ));
    out.push_str(&format!("\nLaunch:  jcode assistant {profile_name}\n"));
    out.push_str(&format!("Resume:  jcode --resume {session_id}\n"));
    out
}

/// `jcode assistant list`
pub(crate) fn run_list(json: bool) -> Result<()> {
    let config = Config::load();
    let profiles = &config.assistant;

    if json {
        let entries: Vec<serde_json::Value> = profiles
            .profiles
            .iter()
            .map(|(name, profile)| {
                serde_json::json!({
                    "name": name,
                    "display_name": profile.display_name_or(name),
                    "cwd": profile.resolved_cwd(),
                    "session_id": assistant_session_id(name, profile),
                    "model": profile.model,
                    "provider": profile.provider,
                    "memory_scope": profile.memory_scope.as_str(),
                    "mode": profile.mode.as_str(),
                    "backing": profile.zmx_session,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if profiles.is_empty() {
        output::stderr_info("No assistant profiles configured.");
        println!("{}", example_config(""));
        return Ok(());
    }

    println!("Assistant profiles:");
    for (name, profile) in &profiles.profiles {
        let backing = profile
            .zmx_session
            .as_deref()
            .map(|b| format!("  (zmx: {b})"))
            .unwrap_or_default();
        println!(
            "  {:<12} {} -> {}{}",
            name,
            profile.display_name_or(name),
            profile.resolved_cwd(),
            backing,
        );
    }
    println!("\nLaunch with: jcode assistant <name>");
    Ok(())
}

/// `jcode assistant status <profile>`
pub(crate) fn run_status(profile_name: &str, json: bool) -> Result<()> {
    let config = Config::load();
    let profile = match resolve_profile(&config, profile_name) {
        Ok(profile) => profile,
        Err(err) => {
            eprintln!("Error: {err}");
            eprintln!();
            eprintln!("{}", example_config(profile_name));
            std::process::exit(1);
        }
    };
    let session_id = assistant_session_id(profile_name, &profile);

    if json {
        let exists = crate::session::session_exists(&session_id);
        let value = serde_json::json!({
            "name": profile_name,
            "display_name": profile.display_name_or(profile_name),
            "cwd": profile.resolved_cwd(),
            "session_id": session_id,
            "session_exists": exists,
            "model": profile.model,
            "provider": profile.provider,
            "memory_scope": profile.memory_scope.as_str(),
            "mode": profile.mode.as_str(),
            "backing": profile.zmx_session,
            "launch": format!("jcode assistant {profile_name}"),
            "resume": format!("jcode --resume {session_id}"),
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    print!(
        "{}",
        render_status_text(profile_name, &profile, &session_id)
    );
    Ok(())
}

/// Parse the trailing args of an external `jcode assistant <profile> [...]`
/// launch into the profile name (first positional that is not a flag).
pub(crate) fn launch_profile_name(launch_args: &[String]) -> Option<String> {
    launch_args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .cloned()
}

/// Resolve a launch profile, ensure its session exists with metadata, then
/// mutate `args` so the standard default launch path resumes it in the right
/// cwd. Returns `false` and prints guidance if the profile cannot be resolved.
pub(crate) fn prepare_launch(args: &mut Args, profile_name: &str) -> Result<bool> {
    let config = Config::load();
    let profile = match resolve_profile(&config, profile_name) {
        Ok(profile) => profile,
        Err(err) => {
            eprintln!("Error: {err}");
            eprintln!();
            eprintln!("{}", example_config(profile_name));
            return Ok(false);
        }
    };

    let cwd = profile.resolved_cwd();
    if !std::path::Path::new(&cwd).is_dir() {
        eprintln!("Error: assistant profile '{profile_name}' cwd does not exist: {cwd}");
        return Ok(false);
    }

    // Switch into the profile cwd now. `args.cwd` is applied earlier in startup,
    // so we change the process directory directly here; this also makes
    // in-repo/self-dev detection in the default launch path see the right dir.
    if let Err(err) = std::env::set_current_dir(&cwd) {
        eprintln!("Error: failed to switch to assistant cwd {cwd}: {err}");
        return Ok(false);
    }

    let session_id = ensure_session(profile_name, &profile)?;

    // Apply the profile to the launch args so the existing default path handles
    // server spawn, cwd switch, and TUI resume.
    args.cwd = Some(cwd);
    args.resume = Some(session_id.clone());
    if args.model.is_none()
        && let Some(model) = profile.model.clone()
    {
        args.model = Some(model);
    }

    output::stderr_info(format!(
        "Launching assistant '{}' in {}",
        profile.display_name_or(profile_name),
        profile.resolved_cwd()
    ));
    if let Some(backing) = profile.zmx_session.as_deref() {
        output::stderr_info(format!("Backing: zmx: {backing}"));
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AssistantProfile;

    fn profile(cwd: &str) -> AssistantProfile {
        AssistantProfile {
            cwd: cwd.to_string(),
            ..AssistantProfile::default()
        }
    }

    #[test]
    fn session_id_is_deterministic_and_safe() {
        let p = profile("/tmp");
        let id1 = assistant_session_id("infra", &p);
        let id2 = assistant_session_id("infra", &p);
        assert_eq!(id1, id2);
        assert_eq!(id1, "session_assistant-infra_assistant_infra");
        assert!(crate::id::extract_session_name(&id1).is_some());
    }

    #[test]
    fn session_id_sanitizes_weird_profile_names() {
        let mut p = profile("/tmp");
        p.session_name_pattern = Some("My Shell!".to_string());
        let id = assistant_session_id("Weird/Name", &p);
        assert!(id.starts_with("session_my-shell_assistant_weird-name"));
        assert!(!id.contains(' '));
        assert!(!id.contains('/'));
    }

    #[test]
    fn build_meta_carries_display_cwd_backing() {
        let mut p = profile("/tmp/work");
        p.display_name = Some("Infra".to_string());
        p.zmx_session = Some("jcode-assistant-infra".to_string());
        let meta = build_session_meta("infra", &p);
        assert_eq!(meta.profile, "infra");
        assert_eq!(meta.display_name.as_deref(), Some("Infra"));
        assert_eq!(meta.cwd.as_deref(), Some("/tmp/work"));
        assert_eq!(meta.backing.as_deref(), Some("jcode-assistant-infra"));
    }

    #[test]
    fn build_meta_carries_persona_from_startup_reminder() {
        let mut p = profile("/tmp/work");
        p.startup_reminder = Some("  You are the infra assistant. Stay in 4nix.  ".to_string());
        let meta = build_session_meta("infra", &p);
        assert_eq!(
            meta.persona.as_deref(),
            Some("You are the infra assistant. Stay in 4nix."),
            "startup_reminder is trimmed and threaded into the session persona"
        );
    }

    #[test]
    fn build_meta_persona_absent_when_reminder_missing_or_blank() {
        let p = profile("/tmp/work");
        assert!(build_session_meta("infra", &p).persona.is_none());

        let mut blank = profile("/tmp/work");
        blank.startup_reminder = Some("   \n  ".to_string());
        assert!(
            build_session_meta("infra", &blank).persona.is_none(),
            "blank startup_reminder does not produce a persona"
        );
    }

    #[test]
    fn launch_profile_name_skips_flags() {
        let args = vec!["--foo".to_string(), "infra".to_string()];
        assert_eq!(launch_profile_name(&args), Some("infra".to_string()));
        assert_eq!(launch_profile_name(&[]), None);
    }

    #[test]
    fn example_config_uses_profile_hint() {
        let text = example_config("jcode");
        assert!(text.contains("[assistant.profiles.jcode]"));
        assert!(text.contains("display_name = \"Jcode\""));
    }
}
