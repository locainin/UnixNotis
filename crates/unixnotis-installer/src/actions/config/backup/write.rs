//! Shared atomic writes for backup-related file updates

use std::fs;
use std::path::Path;

pub(crate) fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    // A sibling temp file avoids leaving a partially written target behind
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp_name = format!("{file_name}.tmp-{}", std::process::id());
    let temp_path = path.with_file_name(temp_name);

    if temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }

    fs::write(&temp_path, contents)?;
    fs::rename(&temp_path, path).inspect_err(|_err| {
        let _ = fs::remove_file(&temp_path);
    })
}
