use super::*;

#[test]
fn desired_nofile_soft_limit_only_raises_when_possible() {
    assert_eq!(desired_nofile_soft_limit(1024, 524_288, 8192), Some(8192));
    assert_eq!(desired_nofile_soft_limit(8192, 524_288, 8192), None);
    assert_eq!(desired_nofile_soft_limit(1024, 4096, 8192), Some(4096));
}

#[cfg(unix)]
#[test]
fn spawn_detached_creates_new_session() {
    use tempfile::NamedTempFile;

    let _guard = crate::storage::lock_test_env();
    let output = NamedTempFile::new().expect("temp file");
    let output_path = output.path().to_string_lossy().to_string();
    let parent_sid = unsafe { libc::getsid(0) };

    let mut cmd = std::process::Command::new(std::env::current_exe().expect("current test binary"));
    cmd.args([
        "--ignored",
        "--exact",
        "platform::platform_tests::spawn_detached_child_probe",
    ])
    .env("JCODE_TEST_OUTPUT", &output_path)
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null());

    let mut child = super::spawn_detached(&mut cmd).expect("spawn detached child");
    let status = child.wait().expect("wait for child");
    assert!(status.success(), "child should exit successfully");

    let probe = std::fs::read_to_string(&output_path).expect("read child pid and sid");
    let mut values = probe.split_whitespace().map(|value| {
        value
            .parse::<u32>()
            .expect("parse detached child probe value")
    });
    let child_pid = values.next().expect("child pid");
    let child_sid = values.next().expect("child sid");

    assert_eq!(child_pid, child.id(), "probe should run in spawned child");
    assert_eq!(
        child_sid,
        child.id(),
        "detached child should lead its own session"
    );
    assert_ne!(
        child_sid as i32, parent_sid,
        "detached child should not share parent session"
    );
}

#[cfg(unix)]
#[test]
#[ignore = "helper process for spawn_detached_creates_new_session"]
fn spawn_detached_child_probe() {
    let output_path = std::env::var_os("JCODE_TEST_OUTPUT").expect("probe output path");
    let pid = std::process::id();
    let sid = unsafe { libc::getsid(0) };
    std::fs::write(output_path, format!("{pid} {sid}\n")).expect("write child pid and sid");
}

#[cfg(unix)]
fn wait_until(mut done: impl FnMut() -> bool, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if done() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

/// A detached (session-leading) process must take its helper descendants with
/// it, so the group signal has to stay the preferred path.
#[cfg(unix)]
#[test]
fn signal_detached_process_tree_terminates_descendant_of_group_leader() {
    use std::process::{Command, Stdio};
    use std::time::Duration;

    let temp = tempfile::tempdir().expect("temp dir");
    let ready = temp.path().join("ready");
    let descendant_pid_path = temp.path().join("descendant.pid");
    let survived = temp.path().join("survived");

    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(
            "sh -c 'sleep 3; : > \"$JCODE_TEST_SURVIVED\"' & \
             echo $! > \"$JCODE_TEST_DESCENDANT_PID\"; \
             : > \"$JCODE_TEST_READY\"; \
             sleep 30",
        )
        .env("JCODE_TEST_READY", &ready)
        .env("JCODE_TEST_DESCENDANT_PID", &descendant_pid_path)
        .env("JCODE_TEST_SURVIVED", &survived)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut leader = super::spawn_detached(&mut cmd).expect("spawn detached leader");
    let leader_pid = leader.id();

    assert!(
        wait_until(|| ready.exists(), Duration::from_secs(10)),
        "detached leader should report ready"
    );
    assert_eq!(
        unsafe { libc::getpgid(leader_pid as i32) },
        leader_pid as i32,
        "detached leader should lead its own process group"
    );

    let descendant_pid: u32 = std::fs::read_to_string(&descendant_pid_path)
        .expect("read descendant pid")
        .trim()
        .parse()
        .expect("parse descendant pid");
    assert!(
        super::is_process_running(descendant_pid),
        "descendant should be running before the signal"
    );

    let scope = super::signal_detached_process_tree(leader_pid, libc::SIGTERM)
        .expect("group signal should reach the detached tree");

    let _ = leader.wait();
    assert!(
        wait_until(
            || !super::is_process_running(descendant_pid),
            Duration::from_secs(10)
        ),
        "descendant should not survive termination of the detached process group"
    );
    assert!(
        !survived.exists(),
        "descendant should not have reached its survival marker"
    );
    assert_eq!(
        scope,
        super::SignalScope::ProcessGroup,
        "a group leader must be signalled as a group, not as a bare process"
    );
}

/// A server that never became a group leader is still alive when the group
/// signal reports ESRCH; the graceful stage must reach it individually.
#[cfg(unix)]
#[test]
fn signal_detached_process_tree_falls_back_to_individual_process_for_sigterm() {
    assert_individual_fallback_kills_non_group_leader(libc::SIGTERM);
}

/// The forced stage uses the same fallback policy as the graceful one, so a
/// stubborn non-leader cannot escape `stop --force`.
#[cfg(unix)]
#[test]
fn signal_detached_process_tree_falls_back_to_individual_process_for_sigkill() {
    assert_individual_fallback_kills_non_group_leader(libc::SIGKILL);
}

#[cfg(unix)]
fn assert_individual_fallback_kills_non_group_leader(signal: i32) {
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};

    // A plain spawn leaves the child in the caller's process group, so its PID
    // is not a PGID -- the shape the sticky-server report hit.
    let mut child = Command::new("/bin/sleep")
        .arg("30")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn non-group-leader child");
    let pid = child.id();

    assert_ne!(
        unsafe { libc::getpgid(pid as i32) },
        pid as i32,
        "child should not lead its own process group"
    );
    let group_only = super::signal_detached_process_group(pid, signal)
        .expect_err("group signal must fail for a non-leader");
    assert_eq!(
        group_only.raw_os_error(),
        Some(libc::ESRCH),
        "group signal should report ESRCH while the process is alive"
    );
    assert!(
        super::is_process_running(pid),
        "child must still be alive after the failed group signal"
    );

    let scope = super::signal_detached_process_tree(pid, signal)
        .expect("fallback should reach the individual process");
    assert_eq!(
        scope,
        super::SignalScope::IndividualProcess,
        "the narrower reach must be reported, not hidden"
    );

    let status = child.wait().expect("wait for signalled child");
    assert_eq!(
        status.signal(),
        Some(signal),
        "child should have been killed by the requested signal"
    );
}

/// Only ESRCH means "no such group". Everything else is a real failure and must
/// not be laundered into a narrower signal.
#[cfg(unix)]
#[test]
fn only_esrch_permits_individual_fallback() {
    for code in [
        libc::EPERM,
        libc::EACCES,
        libc::EINVAL,
        libc::EFAULT,
        libc::EAGAIN,
    ] {
        assert!(
            !super::group_signal_may_fall_back(&std::io::Error::from_raw_os_error(code)),
            "errno {code} must not be treated as a missing process group"
        );
    }
    assert!(super::group_signal_may_fall_back(
        &std::io::Error::from_raw_os_error(libc::ESRCH)
    ));
}

/// A live process we are not allowed to signal must surface EPERM rather than
/// silently retrying at a narrower scope.
#[cfg(unix)]
#[test]
fn permission_denied_group_signal_is_surfaced() {
    if unsafe { libc::getuid() } == 0 {
        eprintln!("skipping: running as root, no unsignalable process exists");
        return;
    }
    let Some(pid) = foreign_group_leader_pid() else {
        eprintln!("skipping: no foreign process-group leader found");
        return;
    };

    let err = super::signal_detached_process_tree(pid, 0)
        .expect_err("signalling a foreign group leader must fail");
    assert_eq!(
        err.raw_os_error(),
        Some(libc::EPERM),
        "EPERM must be surfaced verbatim, got {err}"
    );
    // A surfaced error is the raw group error. Anything that attempted the
    // narrower signal first would report the fallback attempt instead, and a
    // raw OS error carries no message of its own.
    assert!(
        !err.to_string().contains("no process group led by"),
        "EPERM must not be routed through the individual-process fallback: {err}"
    );
}

/// Find a live process-group leader owned by another user (typically root).
#[cfg(unix)]
fn foreign_group_leader_pid() -> Option<u32> {
    let me = unsafe { libc::getuid() };
    let output = std::process::Command::new("/bin/ps")
        .args(["-eo", "pid=,pgid=,uid="])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid: u32 = fields.next()?.parse().ok()?;
            let pgid: u32 = fields.next()?.parse().ok()?;
            let uid: u32 = fields.next()?.parse().ok()?;
            (pid == pgid && pid > 1 && uid != me).then_some(pid)
        })
        .next()
}

/// `kill(-1, ...)` broadcasts to every signalable process and `kill(0, ...)`
/// hits our own group. Neither may ever be reached through a PID argument.
#[cfg(unix)]
#[test]
fn signal_detached_process_tree_refuses_broadcast_pids() {
    for pid in [0, 1] {
        let err = super::signal_detached_process_tree(pid, libc::SIGTERM)
            .expect_err("broadcast pid must be refused");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "pid {pid}");
    }
}

#[cfg(windows)]
#[test]
fn is_process_running_reports_exited_children_as_stopped() {
    use std::process::{Command, Stdio};
    use std::time::Duration;

    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/C", "ping -n 3 127.0.0.1 >NUL"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = cmd.spawn().expect("spawn child");
    let pid = child.id();
    assert!(
        super::is_process_running(pid),
        "child should initially be running"
    );

    let status = child.wait().expect("wait for child");
    assert!(status.success(), "child should exit successfully");
    std::thread::sleep(Duration::from_millis(100));

    assert!(
        !super::is_process_running(pid),
        "exited child should not be reported as running"
    );
}

#[cfg(windows)]
#[test]
fn signal_detached_process_group_terminates_descendant_tree() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let temp = tempfile::tempdir().expect("temp dir");
    let ready_path = temp.path().join("child-ready.txt");
    let survived_path = temp.path().join("child-survived.txt");
    let child_script_path = temp.path().join("child.cmd");
    let parent_script_path = temp.path().join("parent.cmd");
    let child_script = concat!(
        "@echo off\r\n",
        "echo ready>\"%~dp0child-ready.txt\"\r\n",
        "ping -n 6 127.0.0.1 >NUL\r\n",
        "echo survived>\"%~dp0child-survived.txt\"\r\n"
    );
    let parent_script = concat!(
        "@echo off\r\n",
        "start \"\" /B cmd.exe /D /C \"\"%~dp0child.cmd\"\"\r\n",
        "ping -n 30 127.0.0.1 >NUL\r\n"
    );
    std::fs::write(&child_script_path, child_script).expect("write child command script");
    std::fs::write(&parent_script_path, parent_script).expect("write parent command script");
    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/D", "/C"])
        .arg(&parent_script_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut parent = super::spawn_detached(&mut cmd).expect("spawn detached process tree");
    let parent_pid = parent.id();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready_path.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(ready_path.exists(), "descendant should report ready");
    assert!(super::is_process_running(parent_pid));

    super::signal_detached_process_group(parent_pid, 0).expect("terminate process tree");
    let deadline = Instant::now() + Duration::from_secs(10);
    while super::is_process_running(parent_pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = parent.wait();

    assert!(!super::is_process_running(parent_pid), "parent should stop");
    std::thread::sleep(Duration::from_secs(6));
    assert!(
        !survived_path.exists(),
        "descendant should not survive termination of the detached process tree"
    );
}

#[cfg(windows)]
#[test]
fn spawn_replacement_process_returns_without_waiting_for_child_exit() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/C", "ping -n 4 127.0.0.1 >NUL"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let start = Instant::now();
    let mut child = super::spawn_replacement_process(&mut cmd)
        .expect("spawn replacement process should succeed");
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "replacement spawn should not block, took {:?}",
        elapsed
    );
    assert!(
        child.try_wait().expect("poll child status").is_none(),
        "replacement child should still be running immediately after spawn"
    );

    child.kill().ok();
    let _ = child.wait();
}
