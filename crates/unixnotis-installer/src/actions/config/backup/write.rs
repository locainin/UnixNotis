//! Shared atomic writes for backup-related file updates

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub(crate) fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    // A sibling temp file avoids leaving a partially written target behind
    let temp_path = atomic_temp_path(path);
    let mut temp_file = create_atomic_temp_file(&temp_path)?;
    temp_file
        .write_all(contents.as_bytes())
        .inspect_err(|_err| {
            let _ = fs::remove_file(&temp_path);
        })?;
    temp_file.flush().inspect_err(|_err| {
        let _ = fs::remove_file(&temp_path);
    })?;
    drop(temp_file);
    fs::rename(&temp_path, path).inspect_err(|_err| {
        let _ = fs::remove_file(&temp_path);
    })
}

pub(super) fn atomic_temp_path(path: &Path) -> std::path::PathBuf {
    // The name stays beside the target so the final rename remains on the same filesystem
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp_name = format!("{file_name}.tmp-{}", std::process::id());
    path.with_file_name(temp_name)
}

fn create_atomic_temp_file(temp_path: &Path) -> io::Result<fs::File> {
    // create_new refuses existing regular files and symlinks instead of following them
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)
}
