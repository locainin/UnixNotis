//! Atomic regular-file copies for executable and backup installation

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::atomic::publish_file_atomic;
use super::regular::open_regular_file;

/// Copy one regular file into an atomically published destination
///
/// Source and destination ancestors must be real directories. The source mode is applied to the
/// staged file before publication, and existing destination links or special files are rejected
///
/// # Errors
///
/// Returns an error when either path crosses a link, the source is not a regular file, or copying,
/// synchronizing, validating, renaming, or parent-directory synchronization fails
pub fn copy_file_atomic(source: &Path, destination: &Path) -> io::Result<()> {
    // Open once so source bytes and permissions come from the same stable object
    let mut input = open_regular_file(source)?;
    let mode = input.metadata()?.permissions().mode() & 0o777;

    publish_file_atomic(destination, mode, |output| {
        io::copy(&mut input, output)?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "tests/install.rs"]
mod tests;
