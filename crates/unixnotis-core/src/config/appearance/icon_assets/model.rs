//! Icon policy limits and decoded pixel data

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

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
