use jcode_test_lane::{Guard, LaneError, LaneOptions, acquire, read_holder};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const ROLE_ENV: &str = "JCODE_TEST_LANE_TEST_ROLE";
const READY_ENV: &str = "JCODE_TEST_LANE_TEST_READY";
const LABEL_ENV: &str = "JCODE_TEST_LANE_TEST_LABEL";

#[test]
fn process_role() {
    let Ok(role) = std::env::var(ROLE_ENV) else {
        return;
    };
    let path = PathBuf::from(std::env::var_os("JCODE_TEST_LANE_PATH").expect("test lock path"));
    let label = std::env::var(LABEL_ENV).unwrap_or_else(|_| role.clone());
    let options = LaneOptions {
        path,
        label,
        timeout: Some(Duration::from_secs(1)),
        poll: Duration::from_millis(25),
    };

    match role.as_str() {
        "hold" => {
            let guard = acquire(options).expect("holder acquires lane");
            assert!(
                matches!(guard, Guard::Held(_)),
                "unexpected guard: {guard:?}"
            );
            let ready = std::env::var_os(READY_ENV).expect("ready path");
            std::fs::write(ready, b"ready").expect("write ready marker");
            std::thread::sleep(Duration::from_secs(30));
            drop(guard);
        }
        "acquire_once" => match acquire(options) {
            Ok(Guard::Held(_)) => {}
            Ok(other) => panic!("expected held guard, got {other:?}"),
            Err(err @ LaneError::Timeout { .. }) => {
                eprintln!("{err}");
                std::process::exit(75);
            }
            Err(err) => panic!("unexpected acquire error: {err}"),
        },
        "nested" => {
            assert!(matches!(acquire(options).unwrap(), Guard::Nested));
        }
        "bypass" => {
            assert!(matches!(acquire(options).unwrap(), Guard::Bypassed));
        }
        "check_cloexec" => match acquire(options).unwrap() {
            Guard::Held(lock) => assert!(lock.fd_is_cloexec().unwrap()),
            other => panic!("expected held guard, got {other:?}"),
        },
        other => panic!("unknown process role {other}"),
    }
}

#[test]
fn mutual_exclusion() {
    let fixture = Fixture::new("mutual-holder");
    let mut holder = fixture.spawn_holder();

    let blocked = fixture.run("acquire_once", &[]);
    assert_eq!(blocked.status.code(), Some(75), "{blocked:?}");

    kill_sigkill(&mut holder);
    let acquired = fixture.run("acquire_once", &[]);
    assert!(acquired.status.success(), "{acquired:?}");
}

#[test]
fn blocked_child_reports_holder() {
    let fixture = Fixture::new("demo-holder");
    let mut holder = fixture.spawn_holder();

    let blocked = fixture.run("acquire_once", &[]);
    assert_eq!(blocked.status.code(), Some(75), "{blocked:?}");
    let stderr = String::from_utf8_lossy(&blocked.stderr);
    assert!(
        stderr.contains(&holder.id().to_string()),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("demo-holder"), "stderr: {stderr}");
    assert!(stderr.contains("timed out"), "stderr: {stderr}");

    kill_sigkill(&mut holder);
}

#[cfg(unix)]
#[test]
fn crash_releases_lock() {
    let fixture = Fixture::new("crash-holder");
    let mut holder = fixture.spawn_holder();
    kill_sigkill(&mut holder);

    let acquired = fixture.run("acquire_once", &[]);
    assert!(acquired.status.success(), "{acquired:?}");
}

#[test]
fn nested_acquisition_does_not_deadlock() {
    let fixture = Fixture::new("nested-holder");
    let mut holder = fixture.spawn_holder();

    let mut child = fixture.command("nested");
    child.env("JCODE_TEST_LANE_HELD", holder.id().to_string());
    let mut child = child.spawn().expect("spawn nested child");
    let status = wait_with_timeout(&mut child, Duration::from_secs(3))
        .expect("nested child timed out and would self-deadlock");
    assert!(status.success(), "nested child status: {status}");

    kill_sigkill(&mut holder);
}

#[test]
fn bypass_env_skips_lock() {
    let fixture = Fixture::new("bypass-holder");
    let mut holder = fixture.spawn_holder();

    let bypassed = fixture.run("bypass", &[("JCODE_TEST_LANE", "0")]);
    assert!(bypassed.status.success(), "{bypassed:?}");

    kill_sigkill(&mut holder);
}

#[test]
fn metadata_round_trip() {
    let fixture = Fixture::new("metadata-holder");
    let mut holder = fixture.spawn_holder();

    let metadata = read_holder(&fixture.lock_path).expect("holder metadata");
    assert_eq!(metadata.pid, holder.id());
    assert_eq!(metadata.label, "metadata-holder");
    assert!(metadata.start_unix > 0);

    kill_sigkill(&mut holder);
}

#[cfg(unix)]
#[test]
fn stale_metadata_is_overwritten_not_trusted() {
    let stale = Fixture::new("stale-holder");
    let mut stale_holder = stale.spawn_holder();
    let stale_pid = stale_holder.id();
    kill_sigkill(&mut stale_holder);
    assert_eq!(read_holder(&stale.lock_path).unwrap().pid, stale_pid);

    let mut fresh = stale.command("hold");
    let ready = stale.temp.path().join("fresh-ready");
    fresh.env(LABEL_ENV, "fresh-holder").env(READY_ENV, &ready);
    let mut fresh = fresh.spawn().expect("spawn fresh holder");
    wait_for_file(&ready, Duration::from_secs(3));
    let metadata = read_holder(&stale.lock_path).expect("fresh metadata");
    assert_eq!(metadata.pid, fresh.id());
    assert_eq!(metadata.label, "fresh-holder");

    kill_sigkill(&mut fresh);
}

#[cfg(unix)]
#[test]
fn lock_fd_is_cloexec() {
    let fixture = Fixture::new("cloexec-holder");
    let checked = fixture.run("check_cloexec", &[]);
    assert!(checked.status.success(), "{checked:?}");
}

struct Fixture {
    temp: TempDir,
    lock_path: PathBuf,
    label: String,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let lock_path = temp.path().join("test-lane.lock");
        Self {
            temp,
            lock_path,
            label: label.to_string(),
        }
    }

    fn command(&self, role: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().expect("current test binary"));
        command
            .args(["--exact", "process_role", "--nocapture", "--test-threads=1"])
            .env(ROLE_ENV, role)
            .env("JCODE_TEST_LANE_PATH", &self.lock_path)
            .env(LABEL_ENV, &self.label)
            .env_remove("JCODE_TEST_LANE")
            .env_remove("JCODE_TEST_LANE_HELD");
        command
    }

    fn run(&self, role: &str, envs: &[(&str, &str)]) -> Output {
        let mut command = self.command(role);
        command.envs(envs.iter().copied());
        command.output().expect("run child role")
    }

    fn spawn_holder(&self) -> Child {
        let ready = self.temp.path().join("ready");
        let mut command = self.command("hold");
        command
            .env(READY_ENV, &ready)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = command.spawn().expect("spawn holder");
        wait_for_file(&ready, Duration::from_secs(3));
        child
    }
}

fn wait_for_file(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {}", path.display());
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("poll child") {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

#[cfg(unix)]
fn kill_sigkill(child: &mut Child) {
    let result = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGKILL) };
    assert_eq!(
        result,
        0,
        "SIGKILL failed: {}",
        std::io::Error::last_os_error()
    );
    let status = child.wait().expect("wait for killed child");
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(status.signal(), Some(libc::SIGKILL));
}

#[cfg(not(unix))]
fn kill_sigkill(child: &mut Child) {
    child.kill().expect("kill holder");
    child.wait().expect("wait for killed holder");
}
