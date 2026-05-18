use super::*;

#[test]
fn macos_launcher_script_shows_alerts_and_uses_terminal_launcher() {
    let script = macos_launcher_script(
        MacTerminalKind::Ghostty,
        "/tmp/jcode",
        Path::new("/Users/test/Applications/Jcode.app"),
    );
    assert!(script.contains("display alert \"Jcode launch failed\""));
    assert!(script.contains("jcode setup-launcher"));
    assert!(script.contains("/usr/bin/open -na Ghostty"));
    assert!(script.contains("macos-launcher.log"));
}

#[test]
fn macos_launcher_refreshes_when_new_bundle_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_dir = temp.path().join("Jcode.app");
    let legacy_app_dir = temp.path().join("jcode.app");
    let state = SetupHintsState {
        desktop_shortcut_created: true,
        ..SetupHintsState::default()
    };

    assert!(should_refresh_macos_app_launcher_paths(
        &state,
        &app_dir,
        &legacy_app_dir,
    ));
}

#[test]
fn macos_launcher_refreshes_when_legacy_bundle_exists() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_dir = temp.path().join("Jcode.app");
    let legacy_app_dir = temp.path().join("jcode.app");
    std::fs::create_dir_all(&app_dir).expect("create new app dir");
    std::fs::create_dir_all(&legacy_app_dir).expect("create legacy app dir");
    let state = SetupHintsState {
        desktop_shortcut_created: true,
        ..SetupHintsState::default()
    };

    assert!(should_refresh_macos_app_launcher_paths(
        &state,
        &app_dir,
        &legacy_app_dir,
    ));
}

#[test]
fn macos_launcher_refreshes_when_new_bundle_is_plain_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_dir = temp.path().join("Jcode.app");
    let legacy_app_dir = temp.path().join("jcode.app");
    std::fs::write(&app_dir, "broken").expect("write broken launcher file");
    let state = SetupHintsState {
        desktop_shortcut_created: true,
        ..SetupHintsState::default()
    };

    assert!(should_refresh_macos_app_launcher_paths(
        &state,
        &app_dir,
        &legacy_app_dir,
    ));
}

#[test]
fn macos_launcher_refreshes_when_bundle_is_incomplete() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_dir = temp.path().join("Jcode.app");
    let legacy_app_dir = temp.path().join("jcode.app");
    std::fs::create_dir_all(app_dir.join("Contents")).expect("create incomplete bundle");
    std::fs::write(macos_app_launcher_info_plist_path(&app_dir), "plist").expect("write plist");
    let state = SetupHintsState {
        desktop_shortcut_created: true,
        ..SetupHintsState::default()
    };

    assert!(!macos_app_launcher_is_valid(&app_dir));
    assert!(should_refresh_macos_app_launcher_paths(
        &state,
        &app_dir,
        &legacy_app_dir,
    ));
}

#[test]
fn macos_launcher_does_not_refresh_when_new_bundle_exists() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_dir = temp.path().join("Jcode.app");
    let legacy_app_dir = temp.path().join("jcode.app");

    // The legacy/new distinction is purely a case difference in the bundle
    // name. On case-insensitive filesystems (default APFS on macOS, NTFS on
    // Windows, etc.) creating `Jcode.app` also makes `jcode.app` "exist",
    // which would unconditionally flip should_refresh -> true and defeat
    // the test. Detect that and skip with a note rather than asserting a
    // condition that cannot hold on this filesystem.
    if filesystem_is_case_insensitive(temp.path()) {
        eprintln!(
            "skipping macos_launcher_does_not_refresh_when_new_bundle_exists: \
             tempdir at {} is on a case-insensitive filesystem so Jcode.app \
             and jcode.app are the same path",
            temp.path().display()
        );
        return;
    }

    std::fs::create_dir_all(app_dir.join("Contents").join("MacOS")).expect("create new app dir");
    std::fs::write(macos_app_launcher_info_plist_path(&app_dir), "plist").expect("write plist");
    std::fs::write(macos_app_launcher_executable_path(&app_dir), "#!/bin/sh\n")
        .expect("write launcher executable");
    let state = SetupHintsState {
        desktop_shortcut_created: true,
        ..SetupHintsState::default()
    };

    assert!(macos_app_launcher_is_valid(&app_dir));
    assert!(!should_refresh_macos_app_launcher_paths(
        &state,
        &app_dir,
        &legacy_app_dir,
    ));
}

/// Returns true if the filesystem backing `probe_dir` treats path components
/// in a case-insensitive way (so `Foo` and `foo` resolve to the same entry).
///
/// Used only by tests in this module; not exposed outside.
fn filesystem_is_case_insensitive(probe_dir: &Path) -> bool {
    let lower = probe_dir.join(".jcode_case_probe_marker");
    // Best-effort: if we cannot create the probe, assume case-sensitive so
    // the test still runs and surfaces real bugs.
    if std::fs::write(&lower, b"").is_err() {
        return false;
    }
    let upper = probe_dir.join(".JCODE_CASE_PROBE_MARKER");
    let insensitive = upper.exists();
    let _ = std::fs::remove_file(&lower);
    insensitive
}
