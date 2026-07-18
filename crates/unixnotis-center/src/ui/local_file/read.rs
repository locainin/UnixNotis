//! Single-open regular-file reads with a caller-provided byte ceiling

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use rustix::fs::{open, Mode, OFlags};

pub(in crate::ui) fn read_regular_file(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let descriptor = open(
        path,
        // NOFOLLOW rejects last-component links while NONBLOCK avoids waiting on special files
        OFlags::CLOEXEC
            .union(OFlags::NOFOLLOW)
            .union(OFlags::NONBLOCK),
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let mut file = File::from(descriptor);

    // Metadata and content come from the same descriptor even if the path changes later
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local path is not a regular file",
        ));
    }
    validate_file_size(metadata.len(), max_bytes)?;

    let capacity = usize::try_from(metadata.len()).unwrap_or(0);
    read_bytes_limited(&mut file, capacity, max_bytes)
}

fn validate_file_size(size: u64, max_bytes: u64) -> io::Result<()> {
    // Descriptor metadata rejects known-large files before allocating their full size
    if size > max_bytes {
        return Err(byte_limit_error(size, max_bytes));
    }
    Ok(())
}

fn read_bytes_limited(
    reader: &mut impl Read,
    capacity: usize,
    max_bytes: u64,
) -> io::Result<Vec<u8>> {
    let max_capacity = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    let mut bytes = Vec::with_capacity(capacity.min(max_capacity));

    // One extra byte detects a regular file that grew after the metadata snapshot
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > max_bytes {
        return Err(byte_limit_error(observed, max_bytes));
    }
    Ok(bytes)
}

fn byte_limit_error(observed: u64, max_bytes: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("local file exceeds byte limit ({observed} > {max_bytes})"),
    )
}

#[cfg(test)]
#[path = "tests/read.rs"]
mod tests;
