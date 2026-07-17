//! Root-contained icon path parsing and descriptor reads

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use rustix::fs::{openat2, Mode, OFlags, ResolveFlags};

use super::{AssetPolicy, IconAssetError};

pub(super) fn normalize_icon_asset_relative_path(asset: &str) -> Result<PathBuf, IconAssetError> {
    let trimmed = asset.trim();
    if trimmed.is_empty() {
        return Err(IconAssetError::Empty);
    }

    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("file:") || lowered.contains("://") {
        // Local file URIs and remote schemes are both non-portable preset content
        return Err(IconAssetError::Url(trimmed.to_string()));
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(IconAssetError::Absolute(trimmed.to_string()));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                return Err(IconAssetError::ParentTraversal(trimmed.to_string()));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(IconAssetError::Absolute(trimmed.to_string()));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(IconAssetError::Empty);
    }
    Ok(normalized)
}

pub(super) fn validate_icon_asset_extension(
    target: &Path,
    policy: AssetPolicy,
) -> Result<(), IconAssetError> {
    let extension = target
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| IconAssetError::UnsupportedExtension(target.to_path_buf()))?;

    if policy
        .allowed_extensions
        .iter()
        .any(|allowed| extension.eq_ignore_ascii_case(allowed))
    {
        return Ok(());
    }

    Err(IconAssetError::UnsupportedExtension(target.to_path_buf()))
}

pub(super) fn validate_existing_icon_asset(
    config_dir: &Path,
    target: &Path,
    policy: AssetPolicy,
) -> Result<(), IconAssetError> {
    if !target.exists() {
        // Missing optional assets fall back to theme icons at render time
        return Err(IconAssetError::Missing(target.to_path_buf()));
    }

    // Canonical paths close the symlink-escape hole for path-only validation
    let root = config_dir
        .canonicalize()
        .map_err(|error| IconAssetError::Io {
            path: config_dir.to_path_buf(),
            message: error.to_string(),
        })?;
    let canonical_target = target.canonicalize().map_err(|error| IconAssetError::Io {
        path: target.to_path_buf(),
        message: error.to_string(),
    })?;
    if !canonical_target.starts_with(&root) {
        return Err(IconAssetError::EscapesRoot(target.to_path_buf()));
    }

    let metadata = fs::metadata(target).map_err(|error| IconAssetError::Io {
        path: target.to_path_buf(),
        message: error.to_string(),
    })?;
    validate_icon_asset_metadata(target, &metadata, policy)
}

pub(super) fn read_icon_asset_beneath_root(
    config_dir: &Path,
    relative: &Path,
    policy: AssetPolicy,
) -> Result<Vec<u8>, IconAssetError> {
    let root = fs::File::open(config_dir).map_err(|error| IconAssetError::Io {
        path: config_dir.to_path_buf(),
        message: error.to_string(),
    })?;
    let file_fd = openat2(
        &root,
        relative,
        // Nonblocking opens turn FIFOs and devices into ordinary validation failures
        OFlags::CLOEXEC
            .union(OFlags::NOFOLLOW)
            .union(OFlags::NONBLOCK),
        Mode::empty(),
        ResolveFlags::BENEATH
            .union(ResolveFlags::NO_MAGICLINKS)
            .union(ResolveFlags::NO_SYMLINKS),
    )
    .map_err(|error| IconAssetError::Io {
        path: relative.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut file = fs::File::from(file_fd);
    let metadata = file.metadata().map_err(|error| IconAssetError::Io {
        path: relative.to_path_buf(),
        message: error.to_string(),
    })?;
    validate_icon_asset_metadata(relative, &metadata, policy)?;

    // Reading one byte beyond the cap detects growth after the metadata check
    let mut bytes = Vec::new();
    file.by_ref()
        .take(policy.max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| IconAssetError::Io {
            path: relative.to_path_buf(),
            message: error.to_string(),
        })?;
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size.cmp(&policy.max_bytes).is_gt() {
        return Err(IconAssetError::TooLarge {
            path: relative.to_path_buf(),
            size,
            max: policy.max_bytes,
        });
    }
    Ok(bytes)
}

fn validate_icon_asset_metadata(
    target: &Path,
    metadata: &fs::Metadata,
    policy: AssetPolicy,
) -> Result<(), IconAssetError> {
    if !metadata.is_file() {
        return Err(IconAssetError::NotRegularFile(target.to_path_buf()));
    }
    if metadata.len() > policy.max_bytes {
        return Err(IconAssetError::TooLarge {
            path: target.to_path_buf(),
            size: metadata.len(),
            max: policy.max_bytes,
        });
    }
    reject_executable_icon_asset(target, metadata)
}

#[cfg(unix)]
fn reject_executable_icon_asset(
    target: &Path,
    metadata: &fs::Metadata,
) -> Result<(), IconAssetError> {
    use std::os::unix::fs::PermissionsExt;

    // Preset assets are data files, so execute bits are never useful
    if metadata.permissions().mode() & 0o111 != 0 {
        return Err(IconAssetError::Executable(target.to_path_buf()));
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_executable_icon_asset(
    _target: &Path,
    _metadata: &fs::Metadata,
) -> Result<(), IconAssetError> {
    Ok(())
}

/// Validate an icon reference without requiring the target file to exist
///
/// # Errors
///
/// Returns an error when the reference is absolute, escapes the config root, or uses an
/// unsupported extension
pub fn validate_icon_asset_reference(asset: &str) -> Result<(), IconAssetError> {
    // Preset import validates references before the optional asset file exists locally
    let relative = normalize_icon_asset_relative_path(asset)?;
    validate_icon_asset_extension(Path::new(&relative), AssetPolicy::default())
}

#[cfg(test)]
#[path = "tests/path.rs"]
mod tests;
