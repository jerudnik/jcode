//! Small persisted UI preferences that survive restarts and session resumes.
//!
//! These are deliberately separate from the main config file: they capture
//! in-app toggles (like hiding inline images) that the user flips at runtime
//! and expects to stick, without editing `config.toml`.

use serde::{Deserialize, Serialize};

const UI_PREFS_FILE: &str = "ui_preferences.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct UiPreferences {
    #[serde(default)]
    pub version: u8,
    /// Whether inline transcript images render expanded. `None` means the
    /// user never toggled it; default to visible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_images_visible: Option<bool>,
}

fn prefs_path() -> Option<std::path::PathBuf> {
    crate::storage::app_config_dir()
        .ok()
        .map(|dir| dir.join(UI_PREFS_FILE))
}

pub(crate) fn load() -> UiPreferences {
    let Some(path) = prefs_path() else {
        return UiPreferences::default();
    };
    crate::storage::read_json::<UiPreferences>(&path).unwrap_or_default()
}

/// Persisted inline-image visibility, defaulting to visible.
pub(crate) fn inline_images_visible() -> bool {
    load().inline_images_visible.unwrap_or(true)
}

/// Persist the inline-image visibility toggle (load-modify-write so future
/// preference fields survive).
pub(crate) fn save_inline_images_visible(visible: bool) {
    let Some(path) = prefs_path() else {
        return;
    };
    let mut prefs = load();
    prefs.inline_images_visible = Some(visible);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(error) = crate::storage::write_json(&path, &prefs) {
        crate::logging::info(&format!(
            "Failed to persist UI preferences {}: {}",
            path.display(),
            error
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::test_support::with_temp_jcode_home;

    #[test]
    fn inline_images_visibility_round_trips_through_disk() {
        with_temp_jcode_home(|| {
            // Default before any toggle: visible.
            assert!(inline_images_visible());

            save_inline_images_visible(false);
            assert!(!inline_images_visible(), "hidden state should persist");

            save_inline_images_visible(true);
            assert!(inline_images_visible(), "visible state should persist");
        });
    }
}
