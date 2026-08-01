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
use super::pipeline::{MAX_ICON_DIMENSION, MAX_ICON_PIXELS};

// Hard wall-clock deadline for the entire SVG subprocess (parse + render)
const SVG_SUBPROCESS_DEADLINE: Duration = Duration::from_millis(500);
const MAX_SVG_BYTES: usize = 1_024_000;
const MAX_RENDERER_STDERR: usize = 16 * 1024;

pub(super) const fn is_gzip_payload(bytes: &[u8]) -> bool {
    matches!(bytes, [0x1f, 0x8b, ..])
}

pub(super) fn decode_svg_bytes(bytes: &[u8], target: u32) -> Result<RasterImage, String> {
    if target == 0 || target > MAX_ICON_DIMENSION {
        return Err("SVG target dimension exceeds decode limit".to_string());
    }
    let svg_renderer = resolve_svg_renderer()?;
    decode_svg_bytes_with_renderer(bytes, target, &svg_renderer)
}

pub(super) fn decode_svg_bytes_with_renderer(
    bytes: &[u8],
    target: u32,
    svg_renderer: &std::path::Path,
) -> Result<RasterImage, String> {
    if target == 0 || target > MAX_ICON_DIMENSION {
        return Err("SVG target dimension exceeds decode limit".to_string());
    }
    let document = if is_gzip_payload(bytes) {
        Cow::Owned(decompress_svgz_with_limit(bytes, MAX_ICON_BYTES)?)
    } else {
        Cow::Borrowed(bytes)
    };
    if document.len() > MAX_SVG_BYTES {
        return Err("SVG exceeds maximum byte limit".to_string());
    }

    let mut child = Command::new(svg_renderer)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir("/")
        .spawn()
        .map_err(|e| format!("failed to spawn SVG renderer: {e}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to capture child stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture child stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
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
        let mut bytes = Vec::new();
        let _ = stderr
            .take(
                u64::try_from(MAX_RENDERER_STDERR)
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )
            .read_to_end(&mut bytes);
        bytes.truncate(MAX_RENDERER_STDERR);
        String::from_utf8_lossy(&bytes).into_owned()
    });

    // Wait for child with timeout
    let exit_status = match wait_with_timeout(&mut child, SVG_SUBPROCESS_DEADLINE) {
        Ok(status) => status,
        Err(error) => {
            let _ = read_handle.join();
            let _ = stderr_handle.join();
            return Err(error.to_string());
        }
    };

    // Check wall-clock timeout
    if wait_start.elapsed() > SVG_SUBPROCESS_DEADLINE {
        let _ = child.kill();
        let _ = read_handle.join();
        let _ = stderr_handle.join();
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
    // Join the bounded diagnostics reader on success as well, so no helper thread
    // outlives the decoder operation
    let _ = stderr_handle
        .join()
        .map_err(|err| format!("stderr reader panicked: {err:?}"))?;
    let (width, height, rgba_data) = read_result?;

    let expected_len = checked_rgba_len(width, height)?;
    if rgba_data.len() != expected_len {
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

// Production resolves only the sibling binary next to the center executable
pub(super) fn resolve_svg_renderer() -> Result<std::path::PathBuf, String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    let parent = current_exe
        .parent()
        .ok_or("current executable has no parent directory")?;
    let candidate = parent.join("unixnotis-svg-renderer");
    if candidate.exists() {
        return Ok(candidate);
    }
    // Cargo test executables live in target/{debug,release}/deps while the
    // sibling helper stays in the profile directory. Installed binaries do
    // not use a `deps` parent, so this fallback is restricted to that layout
    if parent.file_name() == Some(std::ffi::OsStr::new("deps")) {
        if let Some(profile_dir) = parent.parent() {
            let is_cargo_profile = matches!(
                profile_dir.file_name().and_then(std::ffi::OsStr::to_str),
                Some("debug" | "release")
            );
            let candidate = profile_dir.join("unixnotis-svg-renderer");
            if is_cargo_profile && candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err("unixnotis-svg-renderer binary not found next to center executable".to_string())
}

fn read_stdout(mut stdout: std::process::ChildStdout) -> Result<(u32, u32, Vec<u8>), String> {
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

    let expected_len = checked_rgba_len(width, height)?;

    let mut rgba = vec![0u8; expected_len];
    stdout.read_exact(&mut rgba).map_err(|e| e.to_string())?;
    Ok((width, height, rgba))
}

pub(super) fn checked_rgba_len(width: u32, height: u32) -> Result<usize, String> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| "renderer returned overflowing dimensions".to_string())?;
    if width == 0
        || height == 0
        || width > MAX_ICON_DIMENSION
        || height > MAX_ICON_DIMENSION
        || pixels > MAX_ICON_PIXELS
    {
        return Err("renderer returned oversized image".to_string());
    }
    usize::try_from(pixels)
        .ok()
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "renderer returned oversized image".to_string())
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
