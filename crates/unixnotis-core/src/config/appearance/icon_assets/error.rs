//! Errors returned by icon path validation and decoding

use std::path::PathBuf;

use thiserror::Error;

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
#[path = "tests/error.rs"]
mod tests;
