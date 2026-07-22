use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{self, BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

const FIRST_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const FIRST_REQUEST_POLL_INTERVAL: Duration = Duration::from_millis(10);
static NEXT_SOCKET_ID: AtomicUsize = AtomicUsize::new(0);

pub(super) fn unique_socket_path(tag: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = NEXT_SOCKET_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "jfs-{:x}-{timestamp:x}-{sequence:x}-{tag}.sock",
        std::process::id()
    ))
}

pub(super) fn accept_first_requesting_client(
    listener: &UnixListener,
) -> Result<(BufReader<UnixStream>, UnixStream, Value)> {
    loop {
        let (stream, _) = listener
            .accept()
            .context("fake server failed to accept client")?;
        stream
            .set_nonblocking(true)
            .context("fake server failed to configure client nonblocking mode")?;
        let reader_stream = stream
            .try_clone()
            .context("fake server failed to clone client stream")?;
        let mut reader = BufReader::new(reader_stream);
        let mut first_line = Vec::new();
        let deadline = Instant::now() + FIRST_REQUEST_TIMEOUT;

        loop {
            match reader.fill_buf() {
                Ok(buffer) if buffer.is_empty() => break,
                Ok(buffer) => {
                    let newline = buffer.iter().position(|byte| *byte == b'\n');
                    let bytes_to_consume = newline.map_or(buffer.len(), |index| index + 1);
                    first_line.extend_from_slice(&buffer[..bytes_to_consume]);
                    reader.consume(bytes_to_consume);

                    if newline.is_some() {
                        stream
                            .set_nonblocking(false)
                            .context("fake server failed to restore client blocking mode")?;
                        let first_line = std::str::from_utf8(&first_line)
                            .context("fake server first request was not utf-8")?;
                        let first_request = serde_json::from_str(first_line.trim())?;
                        return Ok((reader, stream, first_request));
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    let now = Instant::now();
                    if now >= deadline {
                        break;
                    }
                    std::thread::sleep((deadline - now).min(FIRST_REQUEST_POLL_INTERVAL));
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionReset
                            | io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::BrokenPipe
                    ) =>
                {
                    break;
                }
                Err(error) => {
                    return Err(error).context("fake server failed reading first request");
                }
            }
        }
    }
}

pub(super) fn read_fake_server_request(reader: &mut BufReader<UnixStream>) -> Result<Value> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(serde_json::from_str(line.trim())?)
}

#[test]
fn stalled_client_is_skipped_after_first_request_timeout() -> Result<()> {
    use std::io::Write;
    use std::sync::mpsc;

    let socket_path = unique_socket_path("fst");
    let listener = UnixListener::bind(&socket_path)?;

    // Queue the stalled connection before the server starts accepting so it is
    // deterministically the first client inspected by the helper.
    let mut stalled_client = UnixStream::connect(&socket_path)?;
    let (result_tx, result_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let result =
            accept_first_requesting_client(&listener).map(|(_reader, _writer, request)| request);
        let _ = result_tx.send(result);
    });

    let mut requesting_client = UnixStream::connect(&socket_path)?;
    requesting_client.write_all(b"{\"type\":\"subscribe\",\"id\":7}\n")?;
    requesting_client.flush()?;

    let result = match result_rx.recv_timeout(FIRST_REQUEST_TIMEOUT + Duration::from_secs(1)) {
        Ok(result) => result,
        Err(error) => {
            // Unblock a regressed helper before failing so the test never leaves
            // a permanently blocked thread behind in the test process.
            let _ = stalled_client.write_all(b"{\"type\":\"stalled\"}\n");
            let _ = stalled_client.flush();
            let _ = server.join();
            let _ = std::fs::remove_file(&socket_path);
            anyhow::bail!("fake server did not skip stalled client: {error}");
        }
    };

    let server_result = server
        .join()
        .map_err(|_| anyhow::anyhow!("fake server thread panicked"));
    let _ = std::fs::remove_file(&socket_path);
    server_result?;
    let result = result?;
    assert_eq!(result["type"], "subscribe");
    assert_eq!(result["id"], 7);
    Ok(())
}
