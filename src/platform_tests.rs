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
    use std::io::{BufRead, BufReader};

    let parent_sid = unsafe { libc::getsid(0) };

    // Have the child block on stdin until the parent has had a chance to
    // observe its session id with libc::getsid(child_pid). This avoids any
    // dependency on a portable user-space tool for retrieving SID:
    // - Linux's procps `ps -o sid=` exists; BSD/macOS `ps` does not.
    // - POSIX module on macOS does not export getsid; perl/python fallback
    //   is fragile across distros.
    // The child prints a single byte to stdout once it is up, so the parent
    // knows the kernel has installed it in its own session before we sample
    // getsid() and then write to its stdin to let it exit.
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c")
        .arg("printf 'R\\n'; IFS= read -r _ignored || true")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    let mut child = super::spawn_detached(&mut cmd).expect("spawn detached child");
    let child_pid = child.id();

    // Wait for the child's "R" handshake so we know it has finished
    // setsid()/detach setup and is parked in the read.
    let mut reader = BufReader::new(child.stdout.take().expect("child stdout"));
    let mut handshake = String::new();
    reader
        .read_line(&mut handshake)
        .expect("read child handshake");
    assert!(
        handshake.starts_with('R'),
        "expected child handshake, got {handshake:?}"
    );

    // Sample the child's session id from the parent before allowing it to
    // exit. getsid() on a still-live PID is the authoritative answer.
    let child_sid = unsafe { libc::getsid(child_pid as libc::pid_t) };
    assert!(
        child_sid > 0,
        "getsid({child_pid}) failed (returned {child_sid}); errno={}",
        std::io::Error::last_os_error()
    );

    // Release the child so wait() doesn't hang.
    drop(child.stdin.take());
    let status = child.wait().expect("wait for child");
    assert!(status.success(), "child should exit successfully");

    assert_eq!(
        child_sid as u32, child_pid,
        "detached child should lead its own session"
    );
    assert_ne!(
        child_sid, parent_sid,
        "detached child should not share parent session"
    );
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
