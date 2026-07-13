//! Widget icon asset path validation

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use image::imageops::FilterType;
use image::{ImageFormat, ImageReader, Limits};
use rustix::fs::{openat2, Mode, OFlags, ResolveFlags};

use thiserror::Error;

pub const DEFAULT_ICON_ASSET_MAX_BYTES: u64 = 2_097_152;
pub const DEFAULT_ICON_ASSET_MAX_WIDTH: u32 = 512;
pub const DEFAULT_ICON_ASSET_MAX_HEIGHT: u32 = 512;
pub const DEFAULT_ICON_ASSET_MAX_PIXELS: u64 = 262_144;
pub const DEFAULT_ICON_ASSET_EXTENSIONS: &[&str] = &["svg", "png", "webp", "jpg", "jpeg"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetPolicy {
    // Per-file cap keeps tiny UI icons from becoming accidental memory pressure
    pub max_bytes: u64,
    // Decoded geometry limits stop compressed images from expanding into oversized buffers
    pub max_width: u32,
    pub max_height: u32,
    pub max_pixels: u64,
    // Extension allowlist stays data-only and avoids treating scripts as images
    pub allowed_extensions: &'static [&'static str],
}

impl Default for AssetPolicy {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_ICON_ASSET_MAX_BYTES,
            max_width: DEFAULT_ICON_ASSET_MAX_WIDTH,
            max_height: DEFAULT_ICON_ASSET_MAX_HEIGHT,
            max_pixels: DEFAULT_ICON_ASSET_MAX_PIXELS,
            allowed_extensions: DEFAULT_ICON_ASSET_EXTENSIONS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedIconAsset {
    // Pixels are captured and decoded before GTK sees them, closing path replacement races
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub premultiplied_alpha: bool,
}

#[derive(Debug, Clone)]
pub struct IconAssetResolver {
    // All widget assets are anchored to the active config file directory
    config_dir: Option<PathBuf>,
    // The policy is stored so tests and future callers can tighten limits in one place
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
    // Parse first so hostile paths never reach filesystem probing
    let relative = normalize_icon_asset_relative_path(asset)?;
    let target = config_dir.join(&relative);
    // Extension is cheap to check before touching metadata or following symlinks
    validate_icon_asset_extension(&target, policy)?;
    // Existing-file validation handles symlink escape, mode, type, and size
    validate_existing_icon_asset(config_dir, &target, policy)?;
    Ok(target)
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

/// Validate and decode icon bytes using the default asset policy
///
/// # Errors
///
/// Returns an error when the reference is invalid, the payload exceeds the size limit, or the
/// image format cannot be decoded safely
pub fn validate_icon_asset_contents(asset: &str, bytes: &[u8]) -> Result<(), IconAssetError> {
    let relative = normalize_icon_asset_relative_path(asset)?;
    let policy = AssetPolicy::default();
    validate_icon_asset_extension(&relative, policy)?;
    if bytes.len() as u64 > policy.max_bytes {
        return Err(IconAssetError::TooLarge {
            path: relative,
            size: bytes.len() as u64,
            max: policy.max_bytes,
        });
    }
    decode_icon_asset(&relative, bytes, policy, None).map(|_| ())
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

fn read_icon_asset_beneath_root(
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
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
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
    if !metadata.is_file() {
        return Err(IconAssetError::NotRegularFile(relative.to_path_buf()));
    }
    if metadata.len() > policy.max_bytes {
        return Err(IconAssetError::TooLarge {
            path: relative.to_path_buf(),
            size: metadata.len(),
            max: policy.max_bytes,
        });
    }
    reject_executable_icon_asset(relative, &metadata)?;

    // Reading one byte beyond the cap detects growth after the metadata check
    let mut bytes = Vec::new();
    file.by_ref()
        .take(policy.max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| IconAssetError::Io {
            path: relative.to_path_buf(),
            message: error.to_string(),
        })?;
    if bytes.len() as u64 > policy.max_bytes {
        return Err(IconAssetError::TooLarge {
            path: relative.to_path_buf(),
            size: bytes.len() as u64,
            max: policy.max_bytes,
        });
    }
    Ok(bytes)
}

fn decode_icon_asset(
    path: &Path,
    bytes: &[u8],
    policy: AssetPolicy,
    render_size: Option<u32>,
) -> Result<ResolvedIconAsset, IconAssetError> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        return decode_svg_icon(path, bytes, policy, render_size);
    }
    decode_raster_icon(path, bytes, policy, render_size)
}

fn decode_raster_icon(
    path: &Path,
    bytes: &[u8],
    policy: AssetPolicy,
    render_size: Option<u32>,
) -> Result<ResolvedIconAsset, IconAssetError> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| decode_error(path, error))?;
    let actual_format = reader
        .format()
        .ok_or_else(|| IconAssetError::InvalidFormat(path.to_path_buf()))?;
    let expected_format = ImageFormat::from_extension(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default(),
    )
    .ok_or_else(|| IconAssetError::InvalidFormat(path.to_path_buf()))?;
    if actual_format != expected_format {
        return Err(IconAssetError::FormatMismatch {
            path: path.to_path_buf(),
            expected: format!("{expected_format:?}"),
            actual: format!("{actual_format:?}"),
        });
    }

    reader.limits(decoder_limits(policy));
    let image = reader.decode().map_err(|error| decode_error(path, error))?;
    validate_dimensions(path, image.width(), image.height(), policy)?;
    let image = if let Some(render_size) = render_size {
        if render_size == 0 {
            return Err(IconAssetError::InvalidRenderSize);
        }
        image.resize(render_size, render_size, FilterType::Lanczos3)
    } else {
        image
    };
    let rgba = image.into_rgba8();
    Ok(ResolvedIconAsset {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
        premultiplied_alpha: false,
    })
}

fn decode_svg_icon(
    path: &Path,
    bytes: &[u8],
    policy: AssetPolicy,
    render_size: Option<u32>,
) -> Result<ResolvedIconAsset, IconAssetError> {
    // SVG image nodes can hide secondary compressed payloads or filesystem references
    if String::from_utf8_lossy(bytes).contains("<image") {
        return Err(IconAssetError::EmbeddedSvgImage(path.to_path_buf()));
    }
    let options = resvg::usvg::Options::default();
    let tree =
        resvg::usvg::Tree::from_data(bytes, &options).map_err(|error| decode_error(path, error))?;
    let source_width = tree.size().width().ceil() as u32;
    let source_height = tree.size().height().ceil() as u32;
    validate_dimensions(path, source_width, source_height, policy)?;

    let max_render = render_size.unwrap_or(source_width.max(source_height));
    if max_render == 0 {
        return Err(IconAssetError::InvalidRenderSize);
    }
    let scale =
        (max_render as f32 / tree.size().width()).min(max_render as f32 / tree.size().height());
    let width = (tree.size().width() * scale).round().max(1.0) as u32;
    let height = (tree.size().height() * scale).round().max(1.0) as u32;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(width, height).ok_or_else(|| IconAssetError::Decode {
            path: path.to_path_buf(),
            message: "could not allocate bounded SVG surface".to_string(),
        })?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(ResolvedIconAsset {
        rgba: pixmap.take(),
        width,
        height,
        premultiplied_alpha: true,
    })
}

fn decoder_limits(policy: AssetPolicy) -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(policy.max_width);
    limits.max_image_height = Some(policy.max_height);
    // Four output bytes per pixel plus a small decoder working allowance
    limits.max_alloc = Some(policy.max_pixels.saturating_mul(8));
    limits
}

fn validate_dimensions(
    path: &Path,
    width: u32,
    height: u32,
    policy: AssetPolicy,
) -> Result<(), IconAssetError> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if width == 0
        || height == 0
        || width > policy.max_width
        || height > policy.max_height
        || pixels > policy.max_pixels
    {
        return Err(IconAssetError::Dimensions {
            path: path.to_path_buf(),
            width,
            height,
            max_width: policy.max_width,
            max_height: policy.max_height,
            max_pixels: policy.max_pixels,
        });
    }
    Ok(())
}

fn decode_error(path: &Path, error: impl std::fmt::Display) -> IconAssetError {
    IconAssetError::Decode {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

#[derive(Debug, Error)]
pub enum IconAssetError {
    #[error("icon assets are disabled because the config directory is unavailable")]
    Disabled,
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
    #[error("icon_asset has no recognized image signature: {0}")]
    InvalidFormat(PathBuf),
    #[error("SVG icon_asset must not embed or reference secondary images: {0}")]
    EmbeddedSvgImage(PathBuf),
    #[error("icon_asset format does not match its extension for {path}: expected {expected}, got {actual}")]
    FormatMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("icon_asset dimensions are unsafe for {path}: {width}x{height}, max {max_width}x{max_height} and {max_pixels} pixels")]
    Dimensions {
        path: PathBuf,
        width: u32,
        height: u32,
        max_width: u32,
        max_height: u32,
        max_pixels: u64,
    },
    #[error("icon render size must be greater than zero")]
    InvalidRenderSize,
    #[error("failed to decode icon_asset {path}: {message}")]
    Decode { path: PathBuf, message: String },
    #[error("failed to inspect icon_asset path {path}: {message}")]
    Io { path: PathBuf, message: String },
}

#[cfg(test)]
#[path = "tests/icon_assets.rs"]
mod tests;
