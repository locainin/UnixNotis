//! Bounded SVG and SVGZ parsing with secondary image loading disabled
//!
//! Uses a subprocess renderer with a wall-clock deadline to prevent
//! CPU exhaustion from pathological SVGs (UNX-4-005).

use std::borrow::Cow;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use flate2::read::GzDecoder;

use super::file::MAX_ICON_BYTES;
use super::model::RasterImage;
use super::pipeline::MAX_ICON_PIXELS;

// Hard wall-clock deadline for the entire SVG subprocess (parse + render)
const SVG_SUBPROCESS_DEADLINE: Duration = Duration::from_millis(500);
const MAX_SVG_BYTES: u32 = 1_024_000;

pub(super) const fn is_gzip_payload(bytes: &[u8]) -> bool {
    matches!(bytes, [0x1f, 0x8b, ..])
}

pub(super) fn decode_svg_bytes(bytes: &[u8], target: u32) -> Result<RasterImage, String> {
    // Compressed documents are expanded under the same source byte ceiling
    let document = if is_gzip_payload(bytes) {
        Cow::Owned(decompress_svgz_with_limit(bytes, MAX_ICON_BYTES)?)
    } else {
        Cow::Borrowed(bytes)
    };

    if document.len() > MAX_SVG_BYTES as usize {
        return Err("SVG exceeds maximum byte limit".to_string());
    }

    let svg_renderer = resolve_svg_renderer()?;

    let mut child = Command::new(svg_renderer)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir("/")
        .spawn()
        .map_err(|e| format!("failed to spawn SVG renderer: {e}"))?;

    let mut stdin = child.stdin.take()
        .ok_or_else(|| "failed to capture child stdin".to_string())?;
    let stdout = child.stdout.take()
        .ok_or_else(|| "failed to capture child stdout".to_string())?;
    let mut stderr = child.stderr.take()
        .ok_or_else(|| "failed to capture child stderr".to_string())?;

    // Binary protocol: u32 target dimension (LE) + SVG bytes (remainder of stdin)
    stdin
        .write_all(&(target).to_le_bytes())
        .map_err(|e| format!("failed to write target size: {e}"))?;
    stdin
        .write_all(&document)
        .map_err(|e| format!("failed to write SVG data: {e}"))?;
    drop(stdin);

    // Drain both stdout and stderr concurrently to avoid pipe deadlock
    let wait_start = std::time::Instant::now();
    let read_handle = std::thread::spawn(move || read_stdout(stdout));
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        buf
    });


    // Wait for child with timeout
    let exit_status = wait_with_timeout(&mut child, SVG_SUBPROCESS_DEADLINE)
        .map_err(|e| e.to_string())?;

    // Check wall-clock timeout
    if wait_start.elapsed() > SVG_SUBPROCESS_DEADLINE {
        let _ = child.kill();
        let _ = read_handle.join();
        return Err("SVG render exceeded time limit".to_string());
    }

    // Check exit status before parsing output; child may have failed with no output
    if !exit_status.success() {
        let _ = read_handle.join();
        let stderr_msg = stderr_handle.join().unwrap_or_default();
        let trimmed = stderr_msg.trim();
        if trimmed.is_empty() {
            return Err("SVG renderer subprocess failed".to_string());
        }
        return Err(format!("SVG renderer subprocess failed: {trimmed}"));
    }

    // Child succeeded; parse the read thread result
    let read_result = read_handle
        .join()
        .map_err(|err| format!("stdout reader panicked: {err:?}"))?;
    let (width, height, rgba_data) = read_result?;

    if rgba_data.len() != (width * height * 4) as usize {
        return Err("SVG renderer returned unexpected byte count".to_string());
    }

    let width_i32 = i32::try_from(width).map_err(|e| e.to_string())?;
    let height_i32 = i32::try_from(height).map_err(|e| e.to_string())?;
    let stride = width_i32
        .checked_mul(4)
        .ok_or_else(|| "SVG row stride exceeds supported size".to_string())?;
    Ok(RasterImage {
        bytes: rgba_data,
        width: width_i32,
        height: height_i32,
        stride,
        premultiplied_alpha: true,
    })
}

// In production, resolve only the sibling binary next to the center executable.
// Test injection is provided by set_svg_renderer_for_test.
fn resolve_svg_renderer() -> Result<std::path::PathBuf, String> {
    // Cargo sets CARGO_BIN_EXE_unixnotis_svg_renderer for integration tests only
    #[cfg(test)]
    {
        if let Some(path) = std::env::var("CARGO_BIN_EXE_unixnotis_svg_renderer")
            .ok()
            .map(std::path::PathBuf::from)
        {
            if path.exists() {
                return Ok(path);
            }
        }
        // Manual override for integration tests that need a specific binary
        if let Some(path) = test_renderer_override() {
            return Ok(path);
        }
    }
    let current_exe =
        std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    let parent = current_exe
        .parent()
        .ok_or("current executable has no parent directory")?;
    let candidate = parent.join("unixnotis-svg-renderer");
    if candidate.exists() {
        return Ok(candidate);
    }
    // During tests, the test binary is a separate executable with a hash suffix.
    // Try walking up to the target directory.
    if let Some(grandparent) = parent.parent() {
        let candidate = grandparent.join("unixnotis-svg-renderer");
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("unixnotis-svg-renderer binary not found next to center executable".to_string())
}

fn read_stdout(
    mut stdout: std::process::ChildStdout,
) -> Result<(u32, u32, Vec<u8>), String> {
    let mut width_bytes = [0u8; 4];
    stdout
        .read_exact(&mut width_bytes)
        .map_err(|e| e.to_string())?;
    let width = u32::from_le_bytes(width_bytes);

    let mut height_bytes = [0u8; 4];
    stdout
        .read_exact(&mut height_bytes)
        .map_err(|e| e.to_string())?;
    let height = u32::from_le_bytes(height_bytes);

    let expected_len = (width * height * 4) as usize;
    if expected_len > MAX_ICON_PIXELS as usize * 4 {
        return Err("renderer returned oversized image".to_string());
    }

    let mut rgba = vec![0u8; expected_len];
    stdout
        .read_exact(&mut rgba)
        .map_err(|e| e.to_string())?;
    Ok((width, height, rgba))
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, std::io::Error> {
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        } else if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "SVG subprocess timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn decompress_svgz_with_limit(bytes: &[u8], max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut decoder = GzDecoder::new(bytes);
    let mut document = Vec::new();
    decoder
        .by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut document)
        .map_err(|error| error.to_string())?;
    if u64::try_from(document.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err("decompressed SVG exceeds icon byte limit".to_string());
    }
    Ok(document)
}

#[cfg(test)]
use test_resolver::test_renderer_override;

#[cfg(test)]
mod test_resolver {
    use std::sync::Mutex;
    use std::sync::OnceLock;

    static TEST_RENDERER_OVERRIDE: OnceLock<Mutex<Option<std::path::PathBuf>>> = OnceLock::new();

    pub(super) fn test_renderer_override() -> Option<std::path::PathBuf> {
        TEST_RENDERER_OVERRIDE
            .get()
            .and_then(|lock| lock.lock().ok())
            .and_then(|guard| guard.clone())
    }
}
