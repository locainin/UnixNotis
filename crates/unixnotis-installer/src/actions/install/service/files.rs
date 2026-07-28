//! Regular service artifact file writes and removals

use std::fs;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::{
    ensure_exact_file, ensure_exact_file_pair, regular_file_contents_equal, remove_empty_directory,
    remove_regular_file, remove_regular_file_pair_if_contents, set_file_mode, write_file_atomic,
    write_file_atomic_preserving_mode, EnsureExactFileOutcome, EnsureExactFilePairOutcome,
    RemoveExactFileOutcome,
};

use crate::paths::format_with_home;
use crate::service_manager::MANAGED_DIRECTORY_MARKER_CONTENTS;

pub(in crate::actions::install::service) fn write_regular_service_file(
    path: &Path,
    contents: &str,
    mode: Option<u32>,
    artifact_label: &str,
) -> Result<bool> {
    // Refuse unsafe existing paths before looking at file contents
    let path_exists = ensure_regular_artifact_file_path(path)?;
    let mode_changed = match mode {
        Some(mode) => {
            #[cfg(unix)]
            {
                // Compare before chmod so reinstall stays quiet when both bytes and mode match
                current_mode(path)? != Some(mode)
            }
            #[cfg(not(unix))]
            {
                return Err(anyhow!(
                    "cannot apply executable mode {} on non-Unix platforms",
                    mode
                ));
            }
        }
        None => false,
    };
    let contents_changed = if path_exists {
        let maximum_size = u64::try_from(contents.len()).unwrap_or(u64::MAX);
        // One no-follow descriptor owns both the size gate and bounded byte comparison
        match regular_file_contents_equal(path, contents.as_bytes(), maximum_size) {
            Ok(equal) => !equal,
            Err(error) if error.kind() == ErrorKind::NotFound => true,
            Err(error) => {
                return Err(error).with_context(|| format!("failed to compare {artifact_label}"));
            }
        }
    } else {
        true
    };

    if contents_changed {
        // Explicit modes keep service scripts independent of process umask
        mode.map_or_else(
            || write_file_atomic_preserving_mode(path, contents.as_bytes(), 0o644),
            |mode| write_file_atomic(path, contents.as_bytes(), mode),
        )
        .with_context(|| format!("failed to write {artifact_label}"))?;
    } else if mode_changed {
        #[cfg(unix)]
        if let Some(mode) = mode {
            // Descriptor-based chmod keeps a swapped pathname from redirecting the update
            set_file_mode(path, mode)
                .with_context(|| format!("failed to chmod {}", format_with_home(path)))?;
        }
    }

    Ok(contents_changed || mode_changed)
}

pub(in crate::actions::install::service) fn write_shared_service_file(
    path: &Path,
    contents: &str,
    mode: Option<u32>,
    artifact_label: &str,
    created_marker: Option<&Path>,
) -> Result<bool> {
    if let Some(marker) = created_marker {
        let outcome = ensure_exact_file_pair(
            path,
            contents.as_bytes(),
            mode.unwrap_or(0o644),
            marker,
            MANAGED_DIRECTORY_MARKER_CONTENTS.as_bytes(),
            0o644,
        )
        .with_context(|| format!("failed to write {artifact_label} and its ownership marker"))?;
        return match outcome {
            EnsureExactFilePairOutcome::Created => Ok(true),
            EnsureExactFilePairOutcome::AlreadyExact
            | EnsureExactFilePairOutcome::AlreadyExactUnowned => Ok(false),
            EnsureExactFilePairOutcome::ContentsMismatch => Err(anyhow!(
                "refusing to overwrite shared service artifact at {}",
                format_with_home(path)
            )),
        };
    }

    let outcome = ensure_exact_file(path, contents.as_bytes(), mode.unwrap_or(0o644))
        .with_context(|| format!("failed to write {artifact_label}"))?;
    match outcome {
        EnsureExactFileOutcome::ContentsMismatch => {
            return Err(anyhow!(
                "refusing to overwrite shared service artifact at {}",
                format_with_home(path)
            ));
        }
        EnsureExactFileOutcome::AlreadyExact => return Ok(false),
        EnsureExactFileOutcome::Created => {}
    }

    Ok(true)
}

pub(in crate::actions::install::service) fn remove_shared_service_file(
    path: &Path,
    created_marker: &Path,
    expected_contents: &str,
) -> Result<bool> {
    let outcome = remove_regular_file_pair_if_contents(
        path,
        expected_contents.as_bytes(),
        created_marker,
        MANAGED_DIRECTORY_MARKER_CONTENTS.as_bytes(),
    )
    .with_context(|| format!("failed to remove {}", format_with_home(path)))?;
    match outcome {
        RemoveExactFileOutcome::Missing | RemoveExactFileOutcome::ContentsMismatch => Ok(false),
        RemoveExactFileOutcome::Removed => {
            remove_empty_shared_layout_dirs(path)?;
            Ok(true)
        }
    }
}

#[cfg(unix)]
pub(in crate::actions::install) fn current_mode(path: &Path) -> Result<Option<u32>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata.permissions().mode() & 0o777)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect {}", format_with_home(path)))
        }
    }
}

fn remove_empty_shared_layout_dirs(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    // s6 default bundle initialization creates default/contents.d beside default/type
    // These directories are removed only when empty so user bundle members are preserved
    remove_dir_if_empty(&parent.join("contents.d"))?;
    remove_dir_if_empty(parent)
}

fn remove_dir_if_empty(path: &Path) -> Result<()> {
    match remove_empty_directory(path) {
        Ok(true | false) => Ok(()),
        Err(err) if matches!(err.kind(), ErrorKind::DirectoryNotEmpty) => Ok(()),
        Err(err) => {
            Err(err).with_context(|| format!("failed to remove {}", format_with_home(path)))
        }
    }
}

pub(in crate::actions::install) fn ensure_regular_artifact_file_path(path: &Path) -> Result<bool> {
    // Existing service files may be replaced only when the old path is a plain file
    match fs::symlink_metadata(path) {
        // Replacing a symlink would write through attacker-controlled filesystem state
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "cannot replace symlink service artifact at {}",
            format_with_home(path)
        )),
        // File artifacts cannot take over directories owned by another backend layout
        Ok(metadata) if metadata.is_dir() => Err(anyhow!(
            "cannot replace directory service artifact at {}",
            format_with_home(path)
        )),
        // Regular files are safe to compare and replace through the atomic writer
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        // Sockets, fifos, and device nodes can block or behave strangely when read
        Ok(_) => Err(anyhow!(
            "cannot replace non-regular service artifact at {}",
            format_with_home(path)
        )),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect {}", format_with_home(path)))
        }
    }
}

pub(in crate::actions::install::service) fn remove_regular_service_file(path: &Path) -> Result<()> {
    // File removal checks the final path again so links are not followed on uninstall
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", format_with_home(path)))?;
    if metadata.file_type().is_symlink() {
        // Removing link artifacts goes through the symlink-specific path with target checks
        return Err(anyhow!(
            "refusing to remove symlink service file at {}",
            format_with_home(path)
        ));
    }
    if !metadata.file_type().is_file() {
        // Directories are handled separately because recursive removal needs an ownership marker
        return Err(anyhow!(
            "refusing to remove non-file service artifact at {}",
            format_with_home(path)
        ));
    }

    if remove_regular_file(path)
        .with_context(|| format!("failed to remove {}", format_with_home(path)))?
    {
        Ok(())
    } else {
        Err(anyhow!(
            "service file disappeared before removal at {}",
            format_with_home(path)
        ))
    }
}
