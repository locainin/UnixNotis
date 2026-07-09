//! Symlink-aware file writes for user-owned config files

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn write_text_preserving_mode(
    path: &Path,
    contents: &str,
    default_mode: u32,
) -> io::Result<()> {
    let mode = existing_mode_or_default(path, default_mode)?;
    write_text_with_mode(path, contents, mode)
}

pub(crate) fn write_text_with_mode(path: &Path, contents: &str, mode: u32) -> io::Result<()> {
    reject_unsafe_write_target(path)?;
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "target path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;

    let (temp_path, mut temp_file) = create_atomic_temp(path)?;
    let result = (|| -> io::Result<()> {
        temp_file.write_all(contents.as_bytes())?;
        temp_file.flush()?;
        #[cfg(unix)]
        {
            // Set the mode before rename so the visible file never appears too permissive
            temp_file.set_permissions(fs::Permissions::from_mode(mode & 0o777))?;
        }
        temp_file.sync_all()?;
        Ok(())
    })();

    if let Err(err) = result {
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }
    drop(temp_file);

    // Re-check immediately before rename so a late symlink swap is not silently followed
    reject_unsafe_write_target(path).inspect_err(|_err| {
        let _ = fs::remove_file(&temp_path);
    })?;
    fs::rename(&temp_path, path).inspect_err(|_err| {
        let _ = fs::remove_file(&temp_path);
    })
}

pub(crate) fn reject_unsafe_write_target(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("refusing to write through symlink {}", path.display()),
                ));
            }
            if metadata.is_file() {
                return Ok(());
            }
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("refusing to overwrite non-file {}", path.display()),
            ))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn existing_mode_or_default(path: &Path, default_mode: u32) -> io::Result<u32> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("refusing to write through symlink {}", path.display()),
                ));
            }
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("refusing to overwrite non-file {}", path.display()),
                ));
            }
            Ok(file_mode(&metadata))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(default_mode),
        Err(err) => Err(err),
    }
}

#[cfg(unix)]
fn file_mode(metadata: &fs::Metadata) -> u32 {
    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn file_mode(_metadata: &fs::Metadata) -> u32 {
    0o644
}

fn create_atomic_temp(path: &Path) -> io::Result<(PathBuf, fs::File)> {
    for attempt in 0..16 {
        let temp_path = atomic_temp_path(path, attempt);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                // Another installer run may have picked the same timestamp; retry with a new suffix
                continue;
            }
            Err(err) => return Err(err),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("could not create temporary file beside {}", path.display()),
    ))
}

fn atomic_temp_path(path: &Path, attempt: u8) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("unixnotis-file");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    path.with_file_name(format!(
        ".{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        stamp,
        attempt
    ))
}
