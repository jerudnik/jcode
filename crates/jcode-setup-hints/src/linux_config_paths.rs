//! Where each Linux compositor keeps the config file jcode splices its launch
//! hotkeys into, plus the XDG base directories those paths derive from.
//!
//! Extracted from `lib.rs`, which is over the code-size budget: these are pure
//! path resolvers with no behaviour, so they read better as a unit and let the
//! parent file shrink. Every `~`-relative lookup goes through
//! `jcode_storage::user_home_path` rather than `dirs::home_dir()`, so a test
//! harness resolves compositor configs inside its sandbox instead of reading
//! the developer's real `~/.config`.
//!
//! The module is declared `#[cfg(any(test, target_os = "linux"))]`, so the
//! items carry no further gates: the previous mix of `linux`-only and
//! `test`-or-`linux` gates meant three of them disappeared on macOS while the
//! parent still imported them by name.

use std::path::PathBuf;

use crate::linux_env::LinuxCompositor;

/// Path to the niri config file, honoring `$XDG_CONFIG_HOME`.
pub(crate) fn niri_config_path() -> Option<PathBuf> {
    Some(xdg_config_home()?.join("niri").join("config.kdl"))
}

/// `$XDG_CONFIG_HOME`, defaulting to `~/.config`.
pub(crate) fn xdg_config_home() -> Option<PathBuf> {
    xdg_config_home_from(std::env::var_os("XDG_CONFIG_HOME"))
}

/// Pure rule behind [`xdg_config_home`], with the ambient env read lifted into
/// an argument so both arms are testable without mutating process env.
fn xdg_config_home_from(xdg: Option<std::ffi::OsString>) -> Option<PathBuf> {
    xdg.map(PathBuf::from)
        .or_else(|| jcode_storage::user_home_path(".config").ok())
}

/// Config file jcode manages for a flat (`#`-commented) compositor config.
/// For i3 the legacy `~/.i3/config` location is honored when the XDG path is
/// missing. GNOME/KDE do not use a spliceable config file and return `None`.
pub(crate) fn flat_compositor_config_path(comp: LinuxCompositor) -> Option<PathBuf> {
    let base = xdg_config_home()?;
    match comp {
        LinuxCompositor::Niri => niri_config_path(),
        LinuxCompositor::Hyprland => Some(base.join("hypr").join("hyprland.conf")),
        LinuxCompositor::Sway => Some(base.join("sway").join("config")),
        LinuxCompositor::Bspwm => Some(base.join("sxhkd").join("sxhkdrc")),
        LinuxCompositor::I3 => {
            let xdg = base.join("i3").join("config");
            if xdg.exists() {
                return Some(xdg);
            }
            let legacy = jcode_storage::user_home_path(".i3/config").ok()?;
            if legacy.exists() {
                Some(legacy)
            } else {
                Some(xdg)
            }
        }
        LinuxCompositor::Gnome
        | LinuxCompositor::Kde
        | LinuxCompositor::Cinnamon
        | LinuxCompositor::Mate
        | LinuxCompositor::Xfce => None,
    }
}

/// KDE's global-shortcuts registry file.
pub(crate) fn kde_globalshortcutsrc_path() -> Option<PathBuf> {
    Some(xdg_config_home()?.join("kglobalshortcutsrc"))
}

/// Directory for jcode's hidden KDE launcher desktop files.
pub(crate) fn kde_applications_dir() -> Option<PathBuf> {
    kde_applications_dir_from(std::env::var_os("XDG_DATA_HOME"))
}

/// Pure rule behind [`kde_applications_dir`]. See [`xdg_config_home_from`].
fn kde_applications_dir_from(xdg_data: Option<std::ffi::OsString>) -> Option<PathBuf> {
    let base = xdg_data
        .map(PathBuf::from)
        .or_else(|| jcode_storage::user_home_path(".local/share").ok())?;
    Some(base.join("applications"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `~`-relative resolver here must land inside the harness sandbox
    /// rather than the developer's real home. Before F29 these called
    /// `dirs::home_dir()`, so running compositor-discovery tests read (and
    /// reported on) whatever niri/sway/i3/KDE config the developer happened to
    /// have installed.
    ///
    /// This asserts the pure resolution rule via the `_from` helpers rather
    /// than by mutating `XDG_CONFIG_HOME`: process-wide env mutation needs a
    /// lease (which lives in `jcode-base`, not a dependency of this crate) and
    /// would make these tests order-dependent for no added coverage.
    #[test]
    fn xdg_fallback_stays_inside_the_redirected_home() {
        let home_relative = [
            ("xdg_config_home", xdg_config_home_from(None)),
            ("kde_applications_dir", kde_applications_dir_from(None)),
        ];

        for (what, path) in home_relative {
            let path = path.unwrap_or_else(|| panic!("{what} resolved to None"));
            jcode_storage::assert_redirected_away_from_real_home(&path, what);
        }
    }

    /// An explicit XDG override wins over the home fallback, so a Linux user
    /// with a non-default layout still gets their own paths.
    #[test]
    fn explicit_xdg_overrides_the_home_fallback() {
        let base = PathBuf::from("/xdg-config");
        assert_eq!(
            xdg_config_home_from(Some(base.clone().into_os_string())),
            Some(base.clone())
        );
        assert_eq!(
            kde_applications_dir_from(Some(PathBuf::from("/xdg-data").into_os_string())),
            Some(PathBuf::from("/xdg-data/applications"))
        );
    }

    /// GNOME/KDE have no spliceable flat config file; the others do.
    #[test]
    fn flat_config_paths_derive_from_the_config_base() {
        let sway = flat_compositor_config_path(LinuxCompositor::Sway)
            .expect("sway has a flat config path");
        jcode_storage::assert_redirected_away_from_real_home(&sway, "sway config path");
        assert!(sway.ends_with("sway/config"), "got {}", sway.display());

        let niri = niri_config_path().expect("niri has a config path");
        assert!(niri.ends_with("niri/config.kdl"), "got {}", niri.display());

        let kde = kde_globalshortcutsrc_path().expect("kde registry path");
        assert!(kde.ends_with("kglobalshortcutsrc"), "got {}", kde.display());

        assert!(flat_compositor_config_path(LinuxCompositor::Gnome).is_none());
    }
}
