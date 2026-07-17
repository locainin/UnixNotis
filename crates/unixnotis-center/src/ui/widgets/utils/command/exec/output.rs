//! Bounded output readers shared by blocking and Tokio workers

use std::io::{self, Read};

use tokio::io::{AsyncRead, AsyncReadExt};

// Command output is diagnostic data, so one MiB per stream is sufficient
const MAX_CAPTURE_BYTES: usize = 1024 * 1024;

pub(super) fn spawn_reader<R: Read + Send + 'static>(
    reader: R,
) -> std::thread::JoinHandle<io::Result<Vec<u8>>> {
    // Dedicated readers prevent a full child pipe from blocking process exit
    std::thread::spawn(move || read_to_end_limited(reader))
}

pub(super) fn join_blocking_reader(
    handle: std::thread::JoinHandle<io::Result<Vec<u8>>>,
    stream: &str,
) -> io::Result<Vec<u8>> {
    match handle.join() {
        Ok(result) => result.map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("failed to read command {stream} stream: {err}"),
            )
        }),
        Err(_) => Err(io::Error::other(format!(
            "command {stream} reader thread panicked"
        ))),
    }
}

pub(super) async fn join_async_reader(
    handle: tokio::task::JoinHandle<io::Result<Vec<u8>>>,
    stream: &str,
) -> io::Result<Vec<u8>> {
    match handle.await {
        Ok(result) => result.map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("failed to read command {stream} stream: {err}"),
            )
        }),
        Err(err) => Err(io::Error::other(format!(
            "command {stream} reader task failed: {err}"
        ))),
    }
}

pub(super) fn read_to_end_limited<R: Read>(reader: R) -> io::Result<Vec<u8>> {
    // Read one byte past the limit so exact-limit output remains valid
    let mut limited = reader.take((MAX_CAPTURE_BYTES as u64) + 1);
    let mut output = Vec::new();
    limited.read_to_end(&mut output)?;
    if output.len() > MAX_CAPTURE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("command output exceeded {MAX_CAPTURE_BYTES} bytes"),
        ));
    }
    Ok(output)
}

pub(super) async fn read_to_end_limited_async<R: AsyncRead + Unpin>(
    reader: R,
) -> io::Result<Vec<u8>> {
    // Blocking and Tokio paths enforce the same byte boundary
    let mut limited = reader.take((MAX_CAPTURE_BYTES as u64) + 1);
    let mut output = Vec::new();
    limited.read_to_end(&mut output).await?;
    if output.len() > MAX_CAPTURE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("command output exceeded {MAX_CAPTURE_BYTES} bytes"),
        ));
    }
    Ok(output)
}
