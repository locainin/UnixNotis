//! Bounded regular-file reads through stable descriptors

use std::io::{self, Read};
use std::path::Path;

use super::atomic::open_regular_file;

/// Read a regular file without following links and enforce a byte limit
///
/// # Errors
///
/// Returns an error when the path crosses a link, the target is not a regular file, the file is
/// larger than `max_bytes`, or the bounded read cannot complete
pub fn read_regular_file_bounded(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    // Opening once keeps the size check and payload read tied to one filesystem object
    let mut file = open_regular_file(path)?;
    let initial_size = file.metadata()?.len();
    if initial_size > max_bytes {
        return Err(limit_error(max_bytes));
    }

    // Reserve only the size already observed and keep the extra-byte growth check bounded
    let capacity = usize::try_from(initial_size).map_err(|_size_error| {
        io::Error::new(io::ErrorKind::InvalidData, "file size does not fit memory")
    })?;
    let mut contents = Vec::with_capacity(capacity);
    file.by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut contents)?;
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(limit_error(max_bytes));
    }
    Ok(contents)
}

fn limit_error(max_bytes: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("regular file exceeds the {max_bytes}-byte limit"),
    )
}

#[cfg(test)]
#[path = "tests/read.rs"]
mod tests;
