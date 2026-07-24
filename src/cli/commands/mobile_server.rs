use anyhow::{Context, Result, bail};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::thread;
use std::time::Duration;

use crate::mobile_server::{self, MobileServerStatus};

pub(crate) fn run_mobile_server_start(port: u16, bind: &str, open_browser: bool) -> Result<()> {
    if let Some(status) = mobile_server::read_running_status() {
        println!(
            "mobile server already running at {} (pid {})",
            status.url, status.pid
        );
        if open_browser {
            open::that(&status.url)?;
        }
        return Ok(());
    }

    let exe = std::env::current_exe().context("resolve current jcode executable")?;
    let log_path = mobile_server::log_path();
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("open {}", log_path.display()))?;
    let log_err = log.try_clone()?;

    let child = ProcessCommand::new(exe)
        .args([
            "mobile-server",
            "serve-internal",
            "--port",
            &port.to_string(),
            "--bind",
            bind,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()
        .context("spawn mobile server")?;

    let pid = child.id();
    for _ in 0..40 {
        if let Some(status) = mobile_server::read_running_status().filter(|s| s.pid == pid) {
            println!(
                "mobile server running at {} (pid {})",
                status.url, status.pid
            );
            if open_browser {
                open::that(&status.url)?;
            }
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    bail!(
        "mobile server did not report ready; see {}",
        log_path.display()
    )
}

pub(crate) fn run_mobile_server_status(json: bool) -> Result<()> {
    let status = mobile_server::read_status();
    if json {
        let body = serde_json::json!({
            "running": status.as_ref().is_some_and(MobileServerStatus::is_running),
            "status": status,
            "status_path": mobile_server::status_path(),
        });
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }

    match status {
        Some(status) if status.is_running() => {
            println!("mobile server: running");
            println!("url: {}", status.url);
            println!("pid: {}", status.pid);
            println!("web root: {}", status.web_root.display());
            println!("log: {}", status.log_path.display());
        }
        Some(status) => {
            println!(
                "mobile server: stopped (stale status for pid {})",
                status.pid
            );
            println!("last url: {}", status.url);
            println!("log: {}", status.log_path.display());
        }
        None => {
            println!("mobile server: stopped");
            println!("status: {}", mobile_server::status_path().display());
            println!("log: {}", mobile_server::log_path().display());
        }
    }
    Ok(())
}

pub(crate) fn run_mobile_server_logs(lines: usize) -> Result<()> {
    let path = mobile_server::log_path();
    let file = File::open(&path).with_context(|| format!("open {}", path.display()))?;
    let all: Vec<String> = BufReader::new(file)
        .lines()
        .collect::<std::io::Result<_>>()?;
    let start = all.len().saturating_sub(lines);
    for line in &all[start..] {
        println!("{}", line);
    }
    Ok(())
}

pub(crate) fn run_mobile_server_stop() -> Result<()> {
    let Some(status) = mobile_server::read_status() else {
        println!("mobile server already stopped");
        return Ok(());
    };
    if status.is_running() {
        terminate_process(status.pid)?;
        println!("stopped mobile server pid {}", status.pid);
    } else {
        println!(
            "mobile server already stopped; clearing stale pid {}",
            status.pid
        );
    }
    mobile_server::clear_status_if_pid(status.pid)?;
    Ok(())
}

pub(crate) fn run_mobile_server_open() -> Result<()> {
    let Some(status) = mobile_server::read_running_status() else {
        bail!("mobile server is not running; start it with `jcode mobile-server start --open`");
    };
    open::that(&status.url)?;
    println!("opened {}", status.url);
    Ok(())
}

pub(crate) fn run_mobile_server_serve_internal(port: u16, bind: &str) -> Result<()> {
    let web_root = mobile_web_root()?;
    let listener =
        TcpListener::bind((bind, port)).with_context(|| format!("bind {bind}:{port}"))?;
    let local_addr = listener.local_addr()?;
    let url = format!("http://{}:{}/", bind, local_addr.port());
    let status = MobileServerStatus {
        pid: std::process::id(),
        port: local_addr.port(),
        bind_addr: bind.to_string(),
        url: url.clone(),
        web_root: web_root.clone(),
        log_path: mobile_server::log_path(),
        started_at_unix: mobile_server::now_unix(),
    };
    mobile_server::write_status(&status)?;
    println!("mobile server ready at {url}");
    println!("serving {}", web_root.display());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &web_root) {
                    eprintln!("request error: {error:#}");
                }
            }
            Err(error) => eprintln!("accept error: {error}"),
        }
    }
    Ok(())
}

/// Relative path, from an installation prefix, to the packaged mobile assets.
/// Follows the FHS `share/` convention so a binary at `<prefix>/bin/jcode`
/// finds its data at `<prefix>/share/jcode/web/jcode-mobile`.
const SHARE_REL: &str = "share/jcode/web/jcode-mobile";
/// Legacy exe-adjacent layout (`<bindir>/web/jcode-mobile`), kept for any
/// tree-style install that co-locates assets next to the binary.
const EXE_ADJACENT_REL: &str = "web/jcode-mobile";
/// In-repo source location, used only as a developer convenience.
const SOURCE_REL: &str = "web/jcode-mobile";

/// Resolve the directory that holds the mobile web assets.
///
/// Resolution is packaging-first so an *installed* binary never depends on the
/// caller's current working directory:
///   1. `JCODE_MOBILE_WEB_ROOT` explicit override (packagers / tests).
///   2. `<prefix>/share/jcode/web/jcode-mobile` derived from the executable
///      (the FHS install layout produced by `nix/package.nix`).
///   3. `<bindir>/web/jcode-mobile` (legacy exe-adjacent tree install).
///   4. `<cwd>/web/jcode-mobile` (developer fallback ONLY, and only when the
///      binary itself is running from inside a source checkout).
///
/// Crucially the CWD candidate is last and gated, so it cannot mask assets
/// missing from a real install: an installed binary launched from a checkout
/// still serves its own packaged assets, and a broken install fails loudly
/// instead of silently serving whatever happens to sit in `./web/jcode-mobile`.
fn mobile_web_root() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("resolve current jcode executable")?;
    // CWD is only a dev fallback, so a failure to read it is not fatal here; it
    // just means the CWD candidate is skipped. Convert the Result to an Option
    // with an explicit match so the discard of the error is visible on purpose.
    let cwd = match std::env::current_dir() {
        Ok(dir) => Some(dir),
        Err(_) => None,
    };
    let env_override = std::env::var_os("JCODE_MOBILE_WEB_ROOT").map(PathBuf::from);
    resolve_mobile_web_root(&exe, cwd.as_deref(), env_override.as_deref(), |p| {
        p.join("index.html").is_file()
    })
    .with_context(|| {
        format!(
            "could not locate mobile web assets; looked for {SHARE_REL} beside the \
             installed binary ({}). Reinstall the package or set JCODE_MOBILE_WEB_ROOT.",
            exe.display()
        )
    })
}

/// Pure resolver behind [`mobile_web_root`], parameterised on the executable
/// path, optional CWD, optional env override, and an `exists` predicate so it
/// is fully unit-testable without touching the filesystem or real env.
fn resolve_mobile_web_root(
    exe: &Path,
    cwd: Option<&Path>,
    env_override: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> Result<PathBuf> {
    // 1. Explicit override wins unconditionally.
    if let Some(root) = env_override {
        if exists(root) {
            return Ok(root.to_path_buf());
        }
        bail!(
            "JCODE_MOBILE_WEB_ROOT={} does not contain index.html",
            root.display()
        );
    }

    let bindir = exe.parent().unwrap_or_else(|| Path::new("."));
    // 2. FHS share path: <prefix>/share/jcode/web/jcode-mobile, where <prefix>
    //    is the parent of <bindir> (i.e. bindir is <prefix>/bin).
    if let Some(prefix) = bindir.parent() {
        let share = prefix.join(SHARE_REL);
        if exists(&share) {
            return Ok(share);
        }
    }
    // 3. Legacy exe-adjacent tree install.
    let adjacent = bindir.join(EXE_ADJACENT_REL);
    if exists(&adjacent) {
        return Ok(adjacent);
    }
    // 4. Developer fallback: CWD, but ONLY when the running binary lives inside
    //    that same checkout. This makes `cargo run` / `target/debug/jcode` work
    //    from the repo while guaranteeing an installed binary can never fall
    //    through to an unrelated `./web/jcode-mobile` and mask a broken install.
    if let Some(cwd) = cwd {
        let source = cwd.join(SOURCE_REL);
        if exists(&source) && exe.starts_with(cwd) {
            return Ok(source);
        }
    }
    bail!("no mobile web asset directory found")
}

fn handle_connection(mut stream: TcpStream, web_root: &Path) -> Result<()> {
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer)?;
    if read == 0 {
        return Ok(());
    }
    let request = String::from_utf8_lossy(&buffer[..read]);
    let first = request.lines().next().unwrap_or_default();
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let raw_path = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        return write_response(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"method not allowed",
            method == "HEAD",
        );
    }
    if raw_path == "/favicon.ico" {
        return write_response(
            &mut stream,
            204,
            "text/plain; charset=utf-8",
            b"",
            method == "HEAD",
        );
    }

    let decoded = percent_decode(raw_path.split('?').next().unwrap_or("/"));
    let relative = decoded.trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    let candidate = web_root.join(relative);
    let file_path = candidate.canonicalize().unwrap_or(candidate);
    let root = web_root.canonicalize()?;
    if !file_path.starts_with(&root) {
        return write_response(
            &mut stream,
            403,
            "text/plain; charset=utf-8",
            b"forbidden",
            method == "HEAD",
        );
    }
    let body = match std::fs::read(&file_path) {
        Ok(body) => body,
        Err(_) => {
            return write_response(
                &mut stream,
                404,
                "text/plain; charset=utf-8",
                b"not found",
                method == "HEAD",
            );
        }
    };
    write_response(
        &mut stream,
        200,
        mime_for(&file_path),
        &body,
        method == "HEAD",
    )
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    head: bool,
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n",
        body.len()
    )?;
    if !head {
        stream.write_all(body)?;
    }
    Ok(())
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn percent_decode(input: &str) -> String {
    urlencoding::decode(input)
        .map(|cow| cow.into_owned())
        .unwrap_or_else(|_| input.to_string())
}

fn terminate_process(pid: u32) -> Result<()> {
    #[cfg(unix)]
    {
        unsafe {
            if libc::kill(pid as libc::pid_t, libc::SIGTERM) != 0 {
                bail!("failed to send SIGTERM to pid {pid}");
            }
        }
        for _ in 0..20 {
            if !mobile_server::process_is_running(pid) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        unsafe {
            let _ = libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        bail!("mobile-server stop is not implemented on this platform yet")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Build an `exists` predicate that returns true only for the given paths.
    fn exists_set(paths: &[&str]) -> impl Fn(&Path) -> bool {
        let set: HashSet<PathBuf> = paths.iter().map(PathBuf::from).collect();
        move |p: &Path| set.contains(p)
    }

    #[test]
    fn share_path_wins_for_installed_binary() {
        // /opt/jcode/bin/jcode -> /opt/jcode/share/jcode/web/jcode-mobile
        let exe = Path::new("/opt/jcode/bin/jcode");
        let cwd = Path::new("/somewhere/else");
        let share = "/opt/jcode/share/jcode/web/jcode-mobile";
        let got = resolve_mobile_web_root(exe, Some(cwd), None, exists_set(&[share])).unwrap();
        assert_eq!(got, PathBuf::from(share));
    }

    #[test]
    fn cwd_cannot_mask_missing_packaged_assets_for_installed_binary() {
        // Installed binary outside any checkout; only the CWD has assets. The
        // resolver must NOT serve them (that would mask a broken install).
        let exe = Path::new("/usr/local/bin/jcode");
        let cwd = Path::new("/home/dev/project");
        let cwd_assets = "/home/dev/project/web/jcode-mobile";
        let err =
            resolve_mobile_web_root(exe, Some(cwd), None, exists_set(&[cwd_assets])).unwrap_err();
        assert!(
            err.to_string()
                .contains("no mobile web asset directory found")
        );
    }

    #[test]
    fn cwd_fallback_allowed_only_when_binary_lives_in_checkout() {
        // `cargo run` case: target/debug/jcode inside the checkout, assets in CWD.
        let exe = Path::new("/home/dev/project/target/debug/jcode");
        let cwd = Path::new("/home/dev/project");
        let cwd_assets = "/home/dev/project/web/jcode-mobile";
        let got = resolve_mobile_web_root(exe, Some(cwd), None, exists_set(&[cwd_assets])).unwrap();
        assert_eq!(got, PathBuf::from(cwd_assets));
    }

    #[test]
    fn env_override_wins_and_is_validated() {
        let exe = Path::new("/opt/jcode/bin/jcode");
        let override_ok = "/custom/assets";
        let got = resolve_mobile_web_root(
            exe,
            None,
            Some(Path::new(override_ok)),
            exists_set(&[override_ok]),
        )
        .unwrap();
        assert_eq!(got, PathBuf::from(override_ok));

        // An override that does not exist is a hard error, never a silent fallthrough.
        let err = resolve_mobile_web_root(
            exe,
            None,
            Some(Path::new("/custom/missing")),
            exists_set(&["/opt/jcode/share/jcode/web/jcode-mobile"]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("JCODE_MOBILE_WEB_ROOT"));
    }

    #[test]
    fn exe_adjacent_layout_is_supported() {
        // Tree install: <bindir>/web/jcode-mobile beside the binary.
        let exe = Path::new("/apps/jcode/jcode");
        let adjacent = "/apps/jcode/web/jcode-mobile";
        let got = resolve_mobile_web_root(exe, None, None, exists_set(&[adjacent])).unwrap();
        assert_eq!(got, PathBuf::from(adjacent));
    }

    #[test]
    fn share_path_preferred_over_cwd_even_inside_checkout() {
        // If a real install layout exists, it wins over the CWD dev fallback.
        let exe = Path::new("/home/dev/project/target/debug/jcode");
        let cwd = Path::new("/home/dev/project");
        let share = "/home/dev/project/target/share/jcode/web/jcode-mobile";
        let cwd_assets = "/home/dev/project/web/jcode-mobile";
        let got = resolve_mobile_web_root(exe, Some(cwd), None, exists_set(&[share, cwd_assets]))
            .unwrap();
        assert_eq!(got, PathBuf::from(share));
    }
}
