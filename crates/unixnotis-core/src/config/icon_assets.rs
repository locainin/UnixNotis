//! Widget icon asset path validation

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

pub const DEFAULT_ICON_ASSET_MAX_BYTES: u64 = 2_097_152;
pub const DEFAULT_ICON_ASSET_EXTENSIONS: &[&str] = &["svg", "png", "webp", "jpg", "jpeg"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetPolicy {
    // Per-file cap keeps tiny UI icons from becoming accidental memory pressure
    pub max_bytes: u64,
    // Extension allowlist stays data-only and avoids treating scripts as images
    pub allowed_extensions: &'static [&'static str],
}

impl Default for AssetPolicy {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_ICON_ASSET_MAX_BYTES,
            allowed_extensions: DEFAULT_ICON_ASSET_EXTENSIONS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IconAssetResolver {
    // All widget assets are anchored to the active config file directory
    config_dir: PathBuf,
    // The policy is stored so tests and future callers can tighten limits in one place
    policy: AssetPolicy,
}

impl IconAssetResolver {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
            policy: AssetPolicy::default(),
        }
    }

    pub fn with_policy(config_dir: impl Into<PathBuf>, policy: AssetPolicy) -> Self {
        Self {
            config_dir: config_dir.into(),
            policy,
        }
    }

    pub fn resolve_icon_asset_path(&self, asset: &str) -> Result<PathBuf, IconAssetError> {
        resolve_icon_asset_path_with_policy(&self.config_dir, asset, self.policy)
    }
}

pub fn resolve_icon_asset_path(config_dir: &Path, asset: &str) -> Result<PathBuf, IconAssetError> {
    resolve_icon_asset_path_with_policy(config_dir, asset, AssetPolicy::default())
}

pub fn resolve_icon_asset_path_with_policy(
    config_dir: &Path,
    asset: &str,
    policy: AssetPolicy,
) -> Result<PathBuf, IconAssetError> {
    // Parse first so hostile paths never reach filesystem probing
    let relative = normalize_icon_asset_relative_path(asset)?;
    let target = config_dir.join(&relative);
    // Extension is cheap to check before touching metadata or following symlinks
    validate_icon_asset_extension(&target, policy)?;
    // Existing-file validation handles symlink escape, mode, type, and size
    validate_existing_icon_asset(config_dir, &target, policy)?;
    Ok(target)
}

pub fn validate_icon_asset_reference(asset: &str) -> Result<(), IconAssetError> {
    // Preset import validates references before the optional asset file exists locally
    let relative = normalize_icon_asset_relative_path(asset)?;
    validate_icon_asset_extension(Path::new(&relative), AssetPolicy::default())
}

fn normalize_icon_asset_relative_path(asset: &str) -> Result<PathBuf, IconAssetError> {
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
            // Dot segments add no useful meaning once the path is config-root relative
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

fn validate_icon_asset_extension(target: &Path, policy: AssetPolicy) -> Result<(), IconAssetError> {
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

fn validate_existing_icon_asset(
    config_dir: &Path,
    target: &Path,
    policy: AssetPolicy,
) -> Result<(), IconAssetError> {
    if !target.exists() {
        // Missing optional assets fall back to theme icons at render time
        return Err(IconAssetError::Missing(target.to_path_buf()));
    }

    // Canonical paths close the symlink-escape hole after the target exists
    let root = config_dir
        .canonicalize()
        .map_err(|err| IconAssetError::Io {
            path: config_dir.to_path_buf(),
            message: err.to_string(),
        })?;
    let canonical_target = target.canonicalize().map_err(|err| IconAssetError::Io {
        path: target.to_path_buf(),
        message: err.to_string(),
    })?;
    if !canonical_target.starts_with(&root) {
        return Err(IconAssetError::EscapesRoot(target.to_path_buf()));
    }

    let metadata = std::fs::metadata(target).map_err(|err| IconAssetError::Io {
        path: target.to_path_buf(),
        message: err.to_string(),
    })?;
    if !metadata.is_file() {
        // GTK image loading should only see regular files, not directories or devices
        return Err(IconAssetError::NotRegularFile(target.to_path_buf()));
    }
    if metadata.len() > policy.max_bytes {
        return Err(IconAssetError::TooLarge {
            path: target.to_path_buf(),
            size: metadata.len(),
            max: policy.max_bytes,
        });
    }
    reject_executable_icon_asset(target, &metadata)?;

    Ok(())
}

#[cfg(unix)]
fn reject_executable_icon_asset(
    target: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), IconAssetError> {
    use std::os::unix::fs::PermissionsExt;

    // Preset assets are data files, not programs, so execute bits are never useful here
    if metadata.permissions().mode() & 0o111 != 0 {
        return Err(IconAssetError::Executable(target.to_path_buf()));
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_executable_icon_asset(
    _target: &Path,
    _metadata: &std::fs::Metadata,
) -> Result<(), IconAssetError> {
    // Non-Unix metadata does not expose POSIX execute bits, so there is nothing to reject
    Ok(())
}

#[derive(Debug, Error)]
pub enum IconAssetError {
    #[error("icon_asset is empty")]
    Empty,
    #[error("icon_asset must be relative to the UnixNotis config directory: {0}")]
    Absolute(String),
    #[error("icon_asset must not use URLs: {0}")]
    Url(String),
    #[error("icon_asset must not contain parent traversal: {0}")]
    ParentTraversal(String),
    #[error("icon_asset uses an unsupported extension: {0}")]
    UnsupportedExtension(PathBuf),
    #[error("icon_asset is missing: {0}")]
    Missing(PathBuf),
    #[error("icon_asset is not a regular file: {0}")]
    NotRegularFile(PathBuf),
    #[error("icon_asset leaves the UnixNotis config directory: {0}")]
    EscapesRoot(PathBuf),
    #[error("icon_asset is too large: {path} ({size} bytes, max {max} bytes)")]
    TooLarge { path: PathBuf, size: u64, max: u64 },
    #[error("icon_asset must not be executable: {0}")]
    Executable(PathBuf),
    #[error("failed to inspect icon_asset path {path}: {message}")]
    Io { path: PathBuf, message: String },
}

#[cfg(test)]
#[path = "tests/icon_assets.rs"]
mod tests;
