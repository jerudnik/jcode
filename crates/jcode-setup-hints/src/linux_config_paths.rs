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
    xdg_config_home_from(
        std::env::var_os("XDG_CONFIG_HOME"),
        jcode_storage::home_is_redirected(),
    )
}

/// Pure rule behind [`xdg_config_home`], with both ambient inputs lifted into
/// arguments so every arm is testable without mutating process env.
///
/// `$XDG_CONFIG_HOME` is *not* derived from the home directory, so the home
/// redirect does not cover it: on a machine where it points at the real
/// `~/.config` (Linux CI does exactly this), honoring it inside a test harness
/// reads the developer's real compositor configs -- the leak this module was
/// extracted to close. So a redirected home wins over the env var, and the env
/// var wins everywhere else.
fn xdg_config_home_from(
    xdg: Option<std::ffi::OsString>,
    home_is_redirected: bool,
) -> Option<PathBuf> {
    if home_is_redirected {
        return jcode_storage::user_home_path(".config").ok();
    }
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
    kde_applications_dir_from(
        std::env::var_os("XDG_DATA_HOME"),
        jcode_storage::home_is_redirected(),
    )
}

/// Pure rule behind [`kde_applications_dir`]. See [`xdg_config_home_from`] for
/// why a redirected home outranks `$XDG_DATA_HOME`.
fn kde_applications_dir_from(
    xdg_data: Option<std::ffi::OsString>,
    home_is_redirected: bool,
) -> Option<PathBuf> {
    let base = if home_is_redirected {
        jcode_storage::user_home_path(".local/share").ok()?
    } else {
        xdg_data
            .map(PathBuf::from)
            .or_else(|| jcode_storage::user_home_path(".local/share").ok())?
    };
    Some(base.join("applications"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug Linux CI caught: CI sets `XDG_CONFIG_HOME` to the real
    /// `~/.config`, so honoring it inside a test harness read the runner's real
    /// compositor configs even though the home was redirected. A redirected
    /// home must outrank the env var.
    #[test]
    fn redirected_home_outranks_an_explicit_xdg_var() {
        let real_looking = Some(std::ffi::OsString::from("/home/runner/.config"));
        let resolved =
            xdg_config_home_from(real_looking, /* home_is_redirected */ true).expect("resolves");
        jcode_storage::assert_redirected_away_from_real_home(&resolved, "xdg_config_home");
        assert!(resolved.ends_with(".config"), "got {}", resolved.display());

        let data = Some(std::ffi::OsString::from("/home/runner/.local/share"));
        let resolved = kde_applications_dir_from(data, true).expect("resolves");
        jcode_storage::assert_redirected_away_from_real_home(&resolved, "kde_applications_dir");
        assert!(
            resolved.ends_with("applications"),
            "got {}",
            resolved.display()
        );
    }

    /// With the real home in play, `$XDG_*` is authoritative: a Linux user with
    /// a non-default layout still gets their own paths.
    #[test]
    fn explicit_xdg_wins_when_the_home_is_not_redirected() {
        assert_eq!(
            xdg_config_home_from(Some("/xdg-config".into()), false),
            Some(PathBuf::from("/xdg-config"))
        );
        assert_eq!(
            kde_applications_dir_from(Some("/xdg-data".into()), false),
            Some(PathBuf::from("/xdg-data/applications"))
        );
    }

    /// With no `$XDG_*` set, both fall back to the (possibly redirected) home.
    #[test]
    fn xdg_fallback_stays_inside_the_redirected_home() {
        for (what, path) in [
            ("xdg_config_home", xdg_config_home_from(None, false)),
            (
                "kde_applications_dir",
                kde_applications_dir_from(None, false),
            ),
        ] {
            let path = path.unwrap_or_else(|| panic!("{what} resolved to None"));
            jcode_storage::assert_redirected_away_from_real_home(&path, what);
        }
    }

    /// The public resolvers must stay inside the sandbox whatever the ambient
    /// environment is, which is what the CI failure was really about: these go
    /// through the env-reading entry points, not the pure helpers.
    #[test]
    fn public_resolvers_never_escape_under_a_harness() {
        let sway = flat_compositor_config_path(LinuxCompositor::Sway)
            .expect("sway has a flat config path");
        jcode_storage::assert_redirected_away_from_real_home(&sway, "sway config path");
        assert!(sway.ends_with("sway/config"), "got {}", sway.display());

        let niri = niri_config_path().expect("niri has a config path");
        jcode_storage::assert_redirected_away_from_real_home(&niri, "niri config path");
        assert!(niri.ends_with("niri/config.kdl"), "got {}", niri.display());

        let kde = kde_globalshortcutsrc_path().expect("kde registry path");
        jcode_storage::assert_redirected_away_from_real_home(&kde, "kde registry path");
        assert!(kde.ends_with("kglobalshortcutsrc"), "got {}", kde.display());

        let apps = kde_applications_dir().expect("kde applications dir");
        jcode_storage::assert_redirected_away_from_real_home(&apps, "kde applications dir");

        // GNOME/KDE have no spliceable flat config; asserted so the checks
        // above cannot silently start accepting `None`.
        assert!(flat_compositor_config_path(LinuxCompositor::Gnome).is_none());
    }
}
