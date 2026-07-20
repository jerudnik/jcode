use jcode_build_support::*;
use chrono::Utc;

fn set_home(p: &std::path::Path) { jcode_core::env::set_var("JCODE_HOME", p); }

fn pending(session: &str, ver: &str, fp: Option<&str>) -> PendingActivation {
    PendingActivation {
        session_id: session.into(),
        new_version: ver.into(),
        previous_current_version: Some("prev-current".into()),
        previous_shared_server_version: Some("prev-shared".into()),
        source_fingerprint: fp.map(str::to_string),
        requested_at: Utc::now() - chrono::Duration::hours(1),
    }
}

fn main() -> anyhow::Result<()> {
    // install_binary_at_version smoke-tests the staged binary by running it
    // with `version --json`. Since we install *ourselves* as the fixture
    // binary, answer that probe and exit instead of recursing.
    if std::env::args().nth(1).as_deref() == Some("version") {
        println!("{{\"version\":\"0.0.0-f11probe\"}}");
        return Ok(());
    }
    let exe = std::env::current_exe()?;

    // Probe 1: corrupt manifest JSON. Expect Err (no action), symlinks untouched,
    // no false rollback, no fabricated state.
    {
        let home = tempfile::tempdir()?;
        set_home(home.path());
        install_binary_at_version(&exe, "prev-current")?;
        update_current_symlink("prev-current")?;
        let mpath = manifest_path()?;
        std::fs::create_dir_all(mpath.parent().unwrap())?;
        std::fs::write(&mpath, b"{ this is not json !!!")?;
        let before = read_current_version()?;
        let res = reconcile_stale_pending_activation(chrono::Duration::minutes(10), |_| false);
        let after = read_current_version()?;
        println!("PROBE1 corrupt-manifest: result={:?} current_before={:?} current_after={:?}", res.as_ref().map(|_| "Ok").map_err(|e| format!("{e:#}")), before, after);
        assert!(res.is_err(), "corrupt manifest must surface an error, not act");
        assert_eq!(before, after, "corrupt manifest must not move symlinks");
        let raw = std::fs::read(&mpath)?;
        assert_eq!(raw, b"{ this is not json !!!", "corrupt manifest must not be overwritten");
        println!("PROBE1 PASS: error surfaced, no symlink movement, manifest untouched");
    }

    // Probe 2: FALSE-ROLLBACK check. Stale pending, dead initiator, candidate
    // binary present, sidecar present, pending.source_fingerprint = None
    // (legacy record without fingerprint). Valid candidate must COMPLETE, never
    // roll back.
    {
        let home = tempfile::tempdir()?;
        set_home(home.path());
        install_binary_at_version(&exe, "prev-current")?;
        install_binary_at_version(&exe, "prev-shared")?;
        let cand = install_binary_at_version(&exe, "cand-nofp")?;
        let meta = DevBinarySourceMetadata {
            version_label: "cand-nofp".into(),
            source_fingerprint: "whatever-fp".into(),
            short_hash: "abc1234".into(),
            full_hash: "abc1234def".into(),
            dirty: false,
            changed_paths: 0,
        };
        std::fs::write(cand.with_file_name(format!("{}.source.json", binary_name())), serde_json::to_vec(&meta)?)?;
        update_current_symlink("cand-nofp")?;
        update_shared_server_symlink("prev-shared")?;
        let mut manifest = BuildManifest::default();
        manifest.set_pending_activation(pending("dead", "cand-nofp", None))?;
        let outcome = reconcile_stale_pending_activation(chrono::Duration::minutes(10), |_| false)?;
        println!("PROBE2 no-fingerprint-valid-candidate: outcome={outcome:?} current={:?}", read_current_version()?);
        assert_eq!(outcome, PendingReconcileOutcome::Completed("cand-nofp".into()), "valid candidate must never be rolled back");
        assert_eq!(read_current_version()?.as_deref(), Some("cand-nofp"));
        println!("PROBE2 PASS: no false rollback for fingerprint-less valid candidate");
    }

    // Probe 3: missing sidecar (binary exists, no .source.json). Unverifiable ->
    // conservative rollback expected; verify symlinks restored, not fabricated.
    {
        let home = tempfile::tempdir()?;
        set_home(home.path());
        install_binary_at_version(&exe, "prev-current")?;
        install_binary_at_version(&exe, "prev-shared")?;
        install_binary_at_version(&exe, "cand-nosc")?;
        update_current_symlink("cand-nosc")?;
        update_shared_server_symlink("cand-nosc")?;
        let mut manifest = BuildManifest::default();
        manifest.set_pending_activation(pending("dead", "cand-nosc", Some("fp")))?;
        let outcome = reconcile_stale_pending_activation(chrono::Duration::minutes(10), |_| false)?;
        println!("PROBE3 missing-sidecar: outcome={outcome:?} current={:?} shared={:?}", read_current_version()?, read_shared_server_version()?);
        assert_eq!(outcome, PendingReconcileOutcome::RolledBack("cand-nosc".into()));
        assert_eq!(read_current_version()?.as_deref(), Some("prev-current"));
        assert_eq!(read_shared_server_version()?.as_deref(), Some("prev-shared"));
        let m = BuildManifest::load()?;
        assert!(m.pending_activation.is_none());
        println!("PROBE3 PASS: unverifiable candidate rolled back to previous, record cleared");
    }

    // Probe 4: idempotency / re-run after completion: second sweep must be NoPending
    // (no double action, no fabricated state on repeat startup).
    {
        let home = tempfile::tempdir()?;
        set_home(home.path());
        install_binary_at_version(&exe, "prev-current")?;
        let cand = install_binary_at_version(&exe, "cand-idem")?;
        let meta = DevBinarySourceMetadata {
            version_label: "cand-idem".into(),
            source_fingerprint: "fp-i".into(),
            short_hash: "abc1234".into(),
            full_hash: "abc1234def".into(),
            dirty: false,
            changed_paths: 0,
        };
        std::fs::write(cand.with_file_name(format!("{}.source.json", binary_name())), serde_json::to_vec(&meta)?)?;
        update_current_symlink("cand-idem")?;
        let mut manifest = BuildManifest::default();
        manifest.set_pending_activation(pending("dead", "cand-idem", Some("fp-i")))?;
        let o1 = reconcile_stale_pending_activation(chrono::Duration::minutes(10), |_| false)?;
        let o2 = reconcile_stale_pending_activation(chrono::Duration::minutes(10), |_| false)?;
        println!("PROBE4 idempotent: first={o1:?} second={o2:?}");
        assert_eq!(o1, PendingReconcileOutcome::Completed("cand-idem".into()));
        assert_eq!(o2, PendingReconcileOutcome::NoPending);
        println!("PROBE4 PASS: second sweep is a no-op");
    }

    println!("ALL F09 PROBES PASS");
    Ok(())
}
