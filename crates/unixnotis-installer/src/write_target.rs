//! Preflight checks for user-owned files edited by the installer

use std::fs;
use std::io;
use std::path::Path;

pub fn reject_unsafe_write_target(path: &Path) -> io::Result<()> {
    // The final component is classified without following a link into another file
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("refusing to write through symlink {}", path.display()),
        )),
        // Existing regular files may proceed to the descriptor-contained writer
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("refusing to overwrite non-file {}", path.display()),
        )),
        // Missing files are valid because the writer creates their parents safely
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
#[path = "tests/write_target.rs"]
mod tests;
