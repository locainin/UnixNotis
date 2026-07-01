//! Service artifact symlink creation and safe removal

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::paths::format_with_home;

pub(in crate::actions::install::service) fn write_service_symlink(
    path: &Path,
    target: &Path,
) -> Result<bool> {
    if let Ok(existing) = fs::read_link(path) {
        if existing == target {
            // Relative links are compared as stored, matching how the backend declared them
            return Ok(false);
        }
        // A different target means another owner may be using this enablement path
        return Err(anyhow!(
            "cannot replace service symlink {} because it points to {} instead of {}",
            format_with_home(path),
            format_with_home(&existing),
            format_with_home(target)
        ));
    } else {
        // Existing non-links are left alone so enablement links cannot overwrite user files
        reject_existing_non_symlink(path)?;
    }

    // Create the link exactly as the backend requested, often with a relative target
    std::os::unix::fs::symlink(target, path)
        .with_context(|| format!("failed to create symlink {}", format_with_home(path)))?;
    Ok(true)
}

pub(in crate::actions::install::service) fn remove_service_symlink(
    path: &Path,
    expected_target: &Path,
) -> Result<()> {
    // Symlink artifacts are removed only when both the type and target still match
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        // Missing links are already gone, which makes uninstall idempotent
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", format_with_home(path)));
        }
    };
    if !metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "refusing to remove non-symlink service artifact at {}",
            format_with_home(path)
        ));
    }

    let actual_target = fs::read_link(path)
        .with_context(|| format!("failed to read symlink {}", format_with_home(path)))?;
    if actual_target != expected_target {
        // A changed link target means ownership is no longer proven
        return Err(anyhow!(
            "refusing to remove symlink {} because it points to {} instead of {}",
            format_with_home(path),
            format_with_home(&actual_target),
            format_with_home(expected_target)
        ));
    }

    fs::remove_file(path).with_context(|| format!("failed to remove {}", format_with_home(path)))
}

fn reject_existing_non_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        // Any existing non-link at the enablement path belongs to the user or another manager
        Ok(_) => Err(anyhow!(
            "cannot replace non-symlink service artifact at {}",
            format_with_home(path)
        )),
        // NotFound means write_service_symlink can safely create the link
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect {}", format_with_home(path)))
        }
    }
}
