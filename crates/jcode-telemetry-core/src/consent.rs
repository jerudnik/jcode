//! Telemetry consent and destination.
//!
//! This fork's privacy policy lives here, isolated from the payload-building
//! and transport machinery in the parent module so that "does jcode send
//! anything, and where" is answerable by reading one short file.
//!
//! Two independent conditions gate every payload: telemetry must be enabled
//! ([`is_enabled`], off by default) *and* an endpoint must be configured
//! ([`telemetry_endpoint`], no default). See `docs/TELEMETRY.md`.

use super::{logging, storage};

/// Whether anonymous usage telemetry may be sent.
///
/// **This fork defaults to off.** Upstream ships opt-*out*: telemetry was
/// enabled unless `JCODE_NO_TELEMETRY`/`DO_NOT_TRACK` was set or a
/// `no_telemetry` marker existed. That sends a persistent install id, OS/arch,
/// provider/model, and session-cadence counters to a third-party endpoint that
/// this fork's maintainer does not operate, from a build the user obtained
/// elsewhere. Defaulting that on is not ours to decide for our users.
///
/// Every upstream *disable* path still works, so anyone carrying a marker file
/// or env var keeps the behavior they configured. Enabling now requires one
/// explicit opt-in: `JCODE_TELEMETRY=1` or a `telemetry_opt_in` marker in the
/// jcode dir. An explicit disable always wins over an explicit enable.
pub fn is_enabled() -> bool {
    if std::env::var("JCODE_NO_TELEMETRY").is_ok() || std::env::var("DO_NOT_TRACK").is_ok() {
        logging::debug("telemetry disabled by environment");
        return false;
    }
    if let Ok(dir) = storage::jcode_dir()
        && dir.join("no_telemetry").exists()
    {
        logging::debug("telemetry disabled by no_telemetry marker");
        return false;
    }
    if env_truthy("JCODE_TELEMETRY") {
        logging::debug("telemetry enabled by JCODE_TELEMETRY");
        return true;
    }
    if let Ok(dir) = storage::jcode_dir()
        && dir.join(TELEMETRY_OPT_IN_MARKER).exists()
    {
        logging::debug("telemetry enabled by opt-in marker");
        return true;
    }
    logging::debug("telemetry off by default (fork policy); opt in with JCODE_TELEMETRY=1");
    false
}

/// Marker file recording that the user opted *in* to anonymous telemetry.
pub(super) const TELEMETRY_OPT_IN_MARKER: &str = "telemetry_opt_in";

/// Treat only explicit affirmative values as opt-in, so `JCODE_TELEMETRY=0`
/// reads as "no" rather than "the variable exists, therefore yes".
fn env_truthy(key: &str) -> bool {
    let Some(value) = std::env::var_os(key) else {
        return false;
    };
    let Some(value) = value.to_str() else {
        logging::warn(&format!("{key} is not valid UTF-8; treating it as unset"));
        return false;
    };
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Marker file recording that the user opted in to sharing prompt and
/// transcript content with telemetry. This is a separate, more sensitive
/// consent than the anonymous usage metrics gated by [`is_enabled`], so it is
/// off by default and only enabled when the user explicitly opts in (e.g. via
/// the first-run onboarding flow).
fn share_content_marker_path() -> Option<std::path::PathBuf> {
    match storage::jcode_dir() {
        Ok(dir) => Some(dir.join("telemetry_share_content")),
        Err(err) => {
            // Fail closed: with no resolvable jcode dir we cannot read the
            // consent marker, and the safe reading of "consent unknown" is
            // "not consented". Log rather than discard, so a broken jcode dir
            // is diagnosable instead of silently presenting as opted out.
            logging::warn(&format!(
                "cannot resolve jcode dir for content-sharing consent ({err}); treating content sharing as off"
            ));
            None
        }
    }
}

/// Whether the user has opted in to sharing prompt/transcript content.
/// Always false when base telemetry is disabled.
pub fn content_sharing_enabled() -> bool {
    if !is_enabled() {
        return false;
    }
    if std::env::var("JCODE_NO_TELEMETRY").is_ok() || std::env::var("DO_NOT_TRACK").is_ok() {
        return false;
    }
    share_content_marker_path().is_some_and(|p| p.exists())
}

/// Persist the user's prompt/transcript content-sharing choice. Writing the
/// marker opts in; removing it opts out. Returns whether the write succeeded.
pub fn set_content_sharing_enabled(enabled: bool) -> bool {
    let Some(path) = share_content_marker_path() else {
        return false;
    };
    if enabled {
        if let Some(parent) = path.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
            && err.kind() != std::io::ErrorKind::AlreadyExists
        {
            // Report rather than discard. The write below would fail too, but
            // with a misleading error about the file when the real problem is
            // the directory.
            logging::warn(&format!(
                "failed to create {} for content-sharing marker: {err}",
                parent.display()
            ));
            return false;
        }
        match std::fs::write(&path, b"1") {
            Ok(()) => {
                logging::debug("telemetry content sharing opted in");
                true
            }
            Err(err) => {
                logging::debug(&format!("failed to write content-sharing marker: {err}"));
                false
            }
        }
    } else {
        match std::fs::remove_file(&path) {
            Ok(()) => true,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
            Err(err) => {
                logging::debug(&format!("failed to remove content-sharing marker: {err}"));
                false
            }
        }
    }
}

/// The endpoint telemetry payloads are posted to, or `None` if none is
/// configured.
///
/// Upstream hardcoded its maintainer's Cloudflare Worker here. This fork does
/// not operate that endpoint, so shipping it as a default would mean an opt-in
/// user's data silently goes to a third party they never chose. There is no
/// default: an operator who wants telemetry sets `JCODE_TELEMETRY_ENDPOINT` to
/// a collector they control, and with nothing set the send path is inert even
/// if telemetry is otherwise enabled.
pub(crate) fn telemetry_endpoint() -> Option<String> {
    // `var_os` rather than `var().ok()`: an unset variable is the expected
    // default here, not an error being discarded, and non-UTF-8 is handled
    // explicitly below rather than silently collapsing into "unset".
    let value = std::env::var_os("JCODE_TELEMETRY_ENDPOINT")?;
    let Some(value) = value.to_str() else {
        logging::warn("JCODE_TELEMETRY_ENDPOINT is not valid UTF-8; ignoring it");
        return None;
    };
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}
