//! Single-open icon file reads with a hard byte ceiling

use std::fs::File;
use std::io::Read;
use std::path::Path;

use rustix::fs::{open, Mode, OFlags};

pub(super) const MAX_ICON_BYTES: u64 = 16 * 1_024 * 1_024;

pub(super) fn read_icon_file(path: &Path) -> Result<Vec<u8>, String> {
    let descriptor = open(
        path,
        // NOFOLLOW rejects last-component links while NONBLOCK avoids waiting on special files
        OFlags::CLOEXEC
            .union(OFlags::NOFOLLOW)
            .union(OFlags::NONBLOCK),
        Mode::empty(),
    )
    .map_err(|error| error.to_string())?;
    let mut file = File::from(descriptor);
    // Metadata and bytes come from the same open file even if the pathname changes later
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("icon path is not a regular file".to_string());
    }
    validate_icon_file_size(metadata.len())?;

    let capacity = usize::try_from(metadata.len()).unwrap_or(0);
    read_icon_bytes(&mut file, capacity, MAX_ICON_BYTES)
}

pub(super) fn validate_icon_file_size(size: u64) -> Result<(), String> {
    // Metadata catches oversized regular files before allocating their buffer
    if size > MAX_ICON_BYTES {
        return Err(format!("icon file too large ({size} bytes)"));
    }
    Ok(())
}

pub(super) fn read_icon_bytes(
    reader: &mut impl Read,
    capacity: usize,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    // Capacity follows trusted descriptor metadata but never exceeds the read ceiling
    let mut bytes = Vec::with_capacity(capacity.min(usize::try_from(max_bytes).unwrap_or(0)));
    // One extra byte detects growth that happened after descriptor metadata was read
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err("icon file too large".to_string());
    }
    Ok(bytes)
}
