//! Service artifact symlink creation and safe removal

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::{remove_symlink_if_target, RemoveSymlinkOutcome};

use crate::paths::format_with_home;

pub(in crate::actions::install) fn write_service_symlink(
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
    }
    // Existing non-links are left alone so enablement links cannot overwrite user files
    reject_existing_non_symlink(path)?;

    // Create the link exactly as the backend requested, often with a relative target
    std::os::unix::fs::symlink(target, path)
        .with_context(|| format!("failed to create symlink {}", format_with_home(path)))?;
    Ok(true)
}

pub(in crate::actions::install) fn remove_service_symlink(
    path: &Path,
    expected_target: &Path,
) -> Result<()> {
    // Core compares the stored target and unlinks relative to the same stable parent descriptor
    match remove_symlink_if_target(path, expected_target) {
        Ok(RemoveSymlinkOutcome::Missing | RemoveSymlinkOutcome::Removed) => Ok(()),
        Ok(RemoveSymlinkOutcome::TargetMismatch(actual_target)) => Err(anyhow!(
            "refusing to remove symlink {} because it points to {} instead of {}",
            format_with_home(path),
            format_with_home(&actual_target),
            format_with_home(expected_target)
        )),
        Err(error) if error.kind() == ErrorKind::InvalidInput => Err(anyhow!(
            "refusing to remove non-symlink service artifact at {}",
            format_with_home(path)
        )),
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to inspect or remove symlink {}",
                format_with_home(path)
            )
        }),
    }
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
