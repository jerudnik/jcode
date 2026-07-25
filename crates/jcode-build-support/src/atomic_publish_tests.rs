//! Atomic-publish regression tests for the stage->fsync->smoke->rename
//! primitive behind the F20b fixed reload path.
//! Extracted from lib.rs to keep that file under the code-size budget; the
//! module still resolves private items via `super::*`.
#![cfg(all(test, unix))]

use super::*;
use std::os::unix::fs::PermissionsExt;

// Serialize on the ONE crate-global publish lock so these tests (which arm the
// process-global INSTALL_STAGE_HOOK) never overlap with resolver/home tests in
// other modules that also drive the publish path.
use super::publish_test_lock as atomic_publish_test_lock;

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

fn write_smoke_script(path: &Path, log_path: Option<&Path>, succeeds: bool) {
    let log_line = log_path
        .map(|path| format!("printf '%s\\n' \"$0\" > {}\n", shell_quote(path)))
        .unwrap_or_default();
    let body = if succeeds {
        format!(
            "#!/bin/sh\n{log_line}if [ \"$1\" = 'version' ] && [ \"$2\" = '--json' ]; then\n  printf '%s\\n' '{{\"version\":\"test-version\",\"git_hash\":\"testhash\"}}'\n  exit 0\nfi\nexit 64\n"
        )
    } else {
        format!("#!/bin/sh\n{log_line}printf '%s\\n' 'boom' >&2\nexit 65\n")
    };
    std::fs::write(path, body).expect("write smoke script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod smoke script");
}

/// Set up a source binary whose staged copy is asserted complete and whose
/// source is truncated mid-stage (via the after-stage hook). Returns the
/// source path, its original bytes, and the smoke-log path.
fn arm_truncation_fixture(dir: &Path) -> (PathBuf, Vec<u8>, PathBuf) {
    let source = dir.join(binary_name());
    let smoke_log = dir.join("smoked-path");
    write_smoke_script(&source, Some(&smoke_log), true);
    let original = std::fs::read(&source).expect("read original script");
    set_after_install_stage_hook({
        let source = source.clone();
        let staged_original = original.clone();
        move |_source, staged| {
            assert_eq!(
                std::fs::read(staged).expect("read staged script"),
                staged_original,
                "staged copy must be complete before the source is truncated"
            );
            std::fs::write(&source, b"").expect("truncate source after staging");
        }
    });
    (source, original, smoke_log)
}

/// Assert the published binary is the complete pre-truncation bytes, the
/// source was truncated, and the smoke test ran the staged temp (not the
/// source, not the final path) in the destination directory.
fn assert_truncation_preserved(
    published: &Path,
    source: &Path,
    original: &[u8],
    smoke_log: &Path,
    dest_dir: &Path,
) {
    assert_eq!(std::fs::metadata(source).expect("source meta").len(), 0);
    assert!(std::fs::metadata(published).expect("published meta").len() > 0);
    assert_eq!(std::fs::read(published).expect("read published"), original);
    let smoked = PathBuf::from(
        std::fs::read_to_string(smoke_log)
            .expect("read smoke log")
            .trim(),
    );
    assert_ne!(&smoked, source, "smoke must not run the source path");
    assert_ne!(&smoked, published, "smoke must run before the rename");
    assert_eq!(smoked.parent(), Some(dest_dir));
    assert!(
        smoked
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(".jcode-publish-")),
        "smoke should run the staged temp in {}",
        dest_dir.display()
    );
}

#[test]
fn fixed_path_publish_survives_source_truncation_between_stage_and_rename() {
    // F20b acceptance gate 1: a mid-stage source truncation still yields a
    // complete published binary at the fixed path, because the staged copy is
    // fully written and smoke-tested before the rename.
    let _guard = atomic_publish_test_lock();
    let fixture = tempfile::tempdir().expect("fixture tempdir");
    let dest_dir = fixture.path().join("current");
    let (source, original, smoke_log) = arm_truncation_fixture(fixture.path());

    let published = atomic_publish_binary(&source, &dest_dir).expect("fixed publish succeeds");

    assert_eq!(published, dest_dir.join(binary_name()));
    assert_truncation_preserved(&published, &source, &original, &smoke_log, &dest_dir);
}

#[test]
fn failed_smoke_test_publishes_nothing_and_leaves_no_staged_temp() {
    // A binary that fails its smoke test must never reach the published path,
    // and must not litter the persistent publish directory with staged temps.
    let _guard = atomic_publish_test_lock();
    let fixture = tempfile::tempdir().expect("fixture tempdir");
    let dest_dir = fixture.path().join("current");
    let source = fixture.path().join(binary_name());
    write_smoke_script(&source, None, false);

    let error = atomic_publish_binary(&source, &dest_dir)
        .expect_err("failed smoke test should reject the publish");

    assert!(error.to_string().contains("Binary smoke test failed"));
    assert!(
        !dest_dir.join(binary_name()).exists(),
        "failed smoke test must not publish a binary"
    );
    let leftovers: Vec<_> = std::fs::read_dir(&dest_dir)
        .expect("read dest dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name())
        .collect();
    assert!(
        leftovers.is_empty(),
        "failed publish must clean up its staged temp, found {leftovers:?}"
    );
}

#[test]
fn failed_publish_preserves_the_previously_published_binary() {
    // The publish directory is persistent: a bad build must not take down the
    // last good binary the daemon can still reload into.
    let _guard = atomic_publish_test_lock();
    let fixture = tempfile::tempdir().expect("fixture tempdir");
    let dest_dir = fixture.path().join("current");

    let good = fixture.path().join("good").join(binary_name());
    std::fs::create_dir_all(good.parent().expect("good dir")).expect("create good dir");
    write_smoke_script(&good, None, true);
    let published = atomic_publish_binary(&good, &dest_dir).expect("first publish succeeds");
    let good_bytes = std::fs::read(&published).expect("read published");

    let bad = fixture.path().join("bad").join(binary_name());
    std::fs::create_dir_all(bad.parent().expect("bad dir")).expect("create bad dir");
    write_smoke_script(&bad, None, false);
    atomic_publish_binary(&bad, &dest_dir).expect_err("bad publish must fail");

    assert_eq!(
        std::fs::read(&published).expect("read published after failure"),
        good_bytes,
        "a failed publish must leave the previously published binary intact"
    );
}
