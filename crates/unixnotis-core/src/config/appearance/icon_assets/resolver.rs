//! Public root-bound icon resolver

use std::path::{Path, PathBuf};

use super::decode::decode_icon_asset;
use super::path::{
    normalize_icon_asset_relative_path, read_icon_asset_beneath_root, validate_existing_icon_asset,
    validate_icon_asset_extension,
};
use super::{AssetPolicy, IconAssetError, ResolvedIconAsset};

#[derive(Debug, Clone)]
pub struct IconAssetResolver {
    // All widget assets are anchored to the active config file directory
    config_dir: Option<PathBuf>,
    // One stored policy keeps path checks and decoders on the same limits
    policy: AssetPolicy,
}

impl IconAssetResolver {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: Some(config_dir.into()),
            policy: AssetPolicy::default(),
        }
    }

    pub fn with_policy(config_dir: impl Into<PathBuf>, policy: AssetPolicy) -> Self {
        Self {
            config_dir: Some(config_dir.into()),
            policy,
        }
    }

    #[must_use]
    pub fn disabled() -> Self {
        // Disabled resolution fails closed while theme-name fallbacks remain available
        Self {
            config_dir: None,
            policy: AssetPolicy::default(),
        }
    }

    /// Resolve an icon reference beneath the configured asset root
    ///
    /// # Errors
    ///
    /// Returns an error when asset resolution is disabled or the reference violates the
    /// configured path, file-type, extension, or size policy
    pub fn resolve_icon_asset_path(&self, asset: &str) -> Result<PathBuf, IconAssetError> {
        let config_dir = self.config_dir.as_deref().ok_or(IconAssetError::Disabled)?;
        resolve_icon_asset_path_with_policy(config_dir, asset, self.policy)
    }

    /// Resolve and decode an icon reference at the requested render size
    ///
    /// # Errors
    ///
    /// Returns an error when the reference is unsafe, the file cannot be read securely, or the
    /// image data cannot be decoded within policy limits
    pub fn resolve_icon_asset(
        &self,
        asset: &str,
        render_size: u32,
    ) -> Result<ResolvedIconAsset, IconAssetError> {
        let config_dir = self.config_dir.as_deref().ok_or(IconAssetError::Disabled)?;
        let relative = normalize_icon_asset_relative_path(asset)?;
        validate_icon_asset_extension(&relative, self.policy)?;
        let bytes = read_icon_asset_beneath_root(config_dir, &relative, self.policy)?;
        decode_icon_asset(&relative, &bytes, self.policy, Some(render_size))
    }
}

/// Resolve an icon reference using the default asset policy
///
/// # Errors
///
/// Returns an error when the reference escapes the config root or targets an unsupported,
/// unsafe, oversized, or invalid file
pub fn resolve_icon_asset_path(config_dir: &Path, asset: &str) -> Result<PathBuf, IconAssetError> {
    resolve_icon_asset_path_with_policy(config_dir, asset, AssetPolicy::default())
}

/// Resolve an icon reference using an explicit asset policy
///
/// # Errors
///
/// Returns an error when the reference or existing file violates the supplied policy
pub fn resolve_icon_asset_path_with_policy(
    config_dir: &Path,
    asset: &str,
    policy: AssetPolicy,
) -> Result<PathBuf, IconAssetError> {
    // Parse before filesystem access so hostile paths never reach metadata probing
    let relative = normalize_icon_asset_relative_path(asset)?;
    let target = config_dir.join(&relative);
    validate_icon_asset_extension(&target, policy)?;
    validate_existing_icon_asset(config_dir, &target, policy)?;
    Ok(target)
}

#[cfg(test)]
#[path = "tests/resolver.rs"]
mod tests;
