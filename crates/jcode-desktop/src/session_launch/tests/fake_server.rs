use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{self, BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};

pub(super) fn accept_first_requesting_client(
    listener: &UnixListener,
) -> Result<(BufReader<UnixStream>, UnixStream, Value)> {
    loop {
        let (stream, _) = listener
            .accept()
            .context("fake server failed to accept client")?;
        let reader_stream = stream
            .try_clone()
            .context("fake server failed to clone client stream")?;
        let mut reader = BufReader::new(reader_stream);
        let mut first_line = String::new();
        match reader.read_line(&mut first_line) {
            Ok(0) => continue,
            Ok(_) => {
                let first_request = serde_json::from_str(first_line.trim())?;
                return Ok((reader, stream, first_request));
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::BrokenPipe
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error).context("fake server failed reading first request"),
        }
    }
}

pub(super) fn read_fake_server_request(reader: &mut BufReader<UnixStream>) -> Result<Value> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(serde_json::from_str(line.trim())?)
}
