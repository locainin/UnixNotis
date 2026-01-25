//! Icon resolution and image helpers for popup rendering.
//!
//! Separates icon lookup and image decoding from UI state management.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use gio::prelude::{AppInfoExt, FileExt};
use gtk::gdk;
use gtk::gdk::prelude::*;
use gtk::{gdk::Texture, IconLookupFlags, IconPaintable, TextDirection};
use image::imageops::FilterType;
use image::GenericImageView;
use unixnotis_core::{NotificationImage, NotificationView};

pub(super) fn file_path_from_hint(path: &str) -> Option<PathBuf> {
    // Accept raw absolute paths and file:// URIs, decoding percent escapes when present.
    if path.starts_with('/') {
        return Some(PathBuf::from(path));
    }
    if path.starts_with("file://") {
        // gio::File handles URI decoding and local filesystem resolution.
        let file = gio::File::for_uri(path);
        // Only accept native filesystem paths to avoid non-local URIs.
        if !file.is_native() {
            return None;
        }
        return file.path();
    }
    None
}

// Resolve themed icon names while filtering out the missing-icon placeholder.
fn resolve_icon_paintable(name: &str, size: i32) -> Option<IconPaintable> {
    if name.is_empty() {
        return None;
    }
    let display = gdk::Display::default()?;
    let icon_theme = gtk::IconTheme::for_display(&display);
    let paintable = icon_theme.lookup_icon(
        name,
        &[],
        size,
        1,
        TextDirection::Ltr,
        IconLookupFlags::empty(),
    );
    if let Some(file) = paintable.file() {
        if let Some(path) = file.path() {
            if is_missing_icon(&path) {
                return None;
            }
        }
    }
    Some(paintable)
}

pub(super) fn resolve_icon_image(name: &str, size: i32) -> Option<gtk::Image> {
    if let Some(file_path) = file_path_from_hint(name) {
        if file_path.is_file() {
            return Some(gtk::Image::from_file(&file_path));
        }
    }
    let paintable = resolve_icon_paintable(name, size)?;
    let widget = gtk::Image::from_paintable(Some(&paintable));
    widget.set_pixel_size(size);
    Some(widget)
}

pub(super) fn collect_icon_candidates(notification: &NotificationView) -> Vec<String> {
    let mut candidates = Vec::new();
    if !notification.image.icon_name.is_empty() {
        candidates.push(notification.image.icon_name.clone());
        if let Some(stripped) = notification.image.icon_name.strip_suffix(".desktop") {
            candidates.push(stripped.to_string());
        }
        candidates.push(notification.image.icon_name.to_lowercase());
    }
    if !notification.app_name.is_empty() {
        candidates.push(notification.app_name.clone());
        let lower = notification.app_name.to_lowercase();
        candidates.push(lower.clone());
        candidates.push(lower.replace(' ', "-"));
    }

    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| !candidate.is_empty() && seen.insert(candidate.clone()))
        .collect()
}

#[derive(Default)]
pub(super) struct DesktopIconIndex {
    by_name: HashMap<String, Vec<String>>,
    by_wm_class: HashMap<String, Vec<String>>,
    by_id: HashMap<String, Vec<String>>,
}

impl DesktopIconIndex {
    pub(super) fn new() -> Self {
        let mut index = Self::default();
        for app_info in gio::AppInfo::all() {
            let Ok(desktop) = app_info.downcast::<gio::DesktopAppInfo>() else {
                continue;
            };
            let icon_name = desktop
                .string("Icon")
                .map(|value| value.to_string())
                .unwrap_or_default();
            if icon_name.is_empty() {
                continue;
            }
            index.add_name(desktop.name().as_str(), &icon_name);
            index.add_name(desktop.display_name().as_str(), &icon_name);
            if let Some(generic) = desktop.generic_name() {
                index.add_name(generic.as_str(), &icon_name);
            }
            if let Some(startup_wm_class) = desktop.startup_wm_class() {
                index.add_wm_class(startup_wm_class.as_str(), &icon_name);
            }
            if let Some(id) = desktop.id() {
                index.add_id(id.as_str(), &icon_name);
            }
        }
        index
    }

    pub(super) fn icons_for(&self, key: &str) -> Option<Vec<String>> {
        let normalized = normalize_key(key);
        if normalized.is_empty() {
            return None;
        }
        let mut out = Vec::new();
        if let Some(values) = self.by_id.get(&normalized) {
            out.extend(values.iter().cloned());
        }
        if let Some(values) = self.by_wm_class.get(&normalized) {
            out.extend(values.iter().cloned());
        }
        if let Some(values) = self.by_name.get(&normalized) {
            out.extend(values.iter().cloned());
        }
        if out.is_empty() {
            return None;
        }
        let mut seen = HashSet::new();
        let filtered = out
            .into_iter()
            .filter(|value| seen.insert(value.clone()))
            .collect::<Vec<_>>();
        Some(filtered)
    }

    fn add_name(&mut self, key: &str, icon: &str) {
        add_icon_to_map(&mut self.by_name, key, icon);
    }

    fn add_wm_class(&mut self, key: &str, icon: &str) {
        add_icon_to_map(&mut self.by_wm_class, key, icon);
    }

    fn add_id(&mut self, key: &str, icon: &str) {
        add_icon_to_map(&mut self.by_id, key, icon);
        if let Some(stripped) = key.strip_suffix(".desktop") {
            add_icon_to_map(&mut self.by_id, stripped, icon);
        }
    }
}

fn add_icon_to_map(map: &mut HashMap<String, Vec<String>>, key: &str, icon: &str) {
    let key = normalize_key(key);
    if key.is_empty() || icon.is_empty() {
        return;
    }
    let entry = map.entry(key).or_default();
    if !entry.iter().any(|value| value == icon) {
        entry.push(icon.to_string());
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn is_missing_icon(path: &Path) -> bool {
    // Filter the theme placeholder to avoid rendering a missing-icon glyph.
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    stem.starts_with("image-missing")
}

pub(super) fn image_data_texture(image: &NotificationImage) -> Option<Texture> {
    if !image.has_image_data {
        return None;
    }
    let data = &image.image_data;
    if data.bits_per_sample != 8 {
        return None;
    }
    // Negative rowstride is invalid for pixel buffers.
    if data.rowstride < 0 {
        return None;
    }

    // Reject non-positive dimensions before creating the texture.
    if data.width <= 0 || data.height <= 0 {
        return None;
    }
    let width = data.width as usize;
    let height = data.height as usize;
    let width_i32 = i32::try_from(width).ok()?;
    let height_i32 = i32::try_from(height).ok()?;

    let (bytes, stride) = match data.channels {
        4 => {
            // Rowstride is bytes per row; hint payloads may include padding.
            let min_stride = width.checked_mul(4)?;
            let stride = if data.rowstride > 0 {
                data.rowstride as usize
            } else {
                min_stride
            };
            // Validate rowstride and buffer length before building the texture.
            if stride < min_stride {
                return None;
            }
            let required = stride.checked_mul(height)?;
            if data.data.len() < required {
                return None;
            }
            (gtk::glib::Bytes::from(&data.data), stride)
        }
        3 => {
            let (expanded, stride) = expand_rgb_to_rgba(data)?;
            (gtk::glib::Bytes::from(&expanded), stride)
        }
        _ => return None,
    };
    Some(
        gdk::MemoryTexture::new(
            width_i32,
            height_i32,
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            stride,
        )
        .upcast::<Texture>(),
    )
}

fn expand_rgb_to_rgba(data: &unixnotis_core::ImageData) -> Option<(Vec<u8>, usize)> {
    // Expand RGB to RGBA while honoring per-row padding in the source buffer.
    let width = usize::try_from(data.width).ok()?;
    let height = usize::try_from(data.height).ok()?;
    if width == 0 || height == 0 {
        return None;
    }

    // Source stride handles optional per-row padding for RGB input.
    let min_src_stride = width.checked_mul(3)?;
    let src_stride = if data.rowstride > 0 {
        data.rowstride as usize
    } else {
        min_src_stride
    };
    if src_stride < min_src_stride {
        return None;
    }
    let required = src_stride.checked_mul(height)?;
    if data.data.len() < required {
        return None;
    }

    // Destination uses tightly packed RGBA rows.
    let dst_stride = width.checked_mul(4)?;
    let mut rgba = vec![0u8; dst_stride.checked_mul(height)?];

    // Copy RGB per pixel and append opaque alpha.
    for y in 0..height {
        let src_row_start = y * src_stride;
        let dst_row_start = y * dst_stride;
        let src_row = &data.data[src_row_start..src_row_start + min_src_stride];
        let dst_row = &mut rgba[dst_row_start..dst_row_start + dst_stride];
        for x in 0..width {
            let src = x * 3;
            let dst = x * 4;
            dst_row[dst] = src_row[src];
            dst_row[dst + 1] = src_row[src + 1];
            dst_row[dst + 2] = src_row[src + 2];
            dst_row[dst + 3] = 255;
        }
    }

    Some((rgba, dst_stride))
}

#[cfg(test)]
mod tests {
    use super::expand_rgb_to_rgba;
    use unixnotis_core::ImageData;

    #[test]
    fn expand_rgb_to_rgba_appends_alpha() {
        // Ensures RGB input is expanded to RGBA with an opaque alpha channel.
        let data = ImageData {
            width: 2,
            height: 1,
            rowstride: 0,
            has_alpha: false,
            bits_per_sample: 8,
            channels: 3,
            data: vec![10, 20, 30, 40, 50, 60],
        };
        let (expanded, stride) = expand_rgb_to_rgba(&data).expect("rgb expansion");
        assert_eq!(stride, 8);
        assert_eq!(expanded, vec![10, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn expand_rgb_to_rgba_honors_row_padding() {
        // Confirms per-row padding does not leak into the expanded RGBA output.
        let data = ImageData {
            width: 2,
            height: 2,
            rowstride: 8,
            has_alpha: false,
            bits_per_sample: 8,
            channels: 3,
            data: vec![1, 2, 3, 4, 5, 6, 0, 0, 7, 8, 9, 10, 11, 12, 0, 0],
        };
        let (expanded, stride) = expand_rgb_to_rgba(&data).expect("rgb expansion");
        assert_eq!(stride, 8);
        assert_eq!(
            expanded,
            vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
        );
    }
}

#[derive(Clone)]
pub(super) struct RasterIcon {
    pub(super) bytes: Vec<u8>,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) stride: i32,
}

// Keep icon loads lightweight; popups only render small thumbnails.
const MAX_ICON_BYTES: u64 = 16 * 1024 * 1024;
// Cap decoded dimensions to bound memory use (512px RGBA = ~1MB per icon).
const MAX_ICON_DIMENSION: u32 = 512;

pub(super) fn decode_icon_file(path: &Path) -> Result<RasterIcon, String> {
    // Decode on a worker thread; keep I/O and CPU-bound work off the GTK main loop.
    let metadata = fs::metadata(path).map_err(|err| err.to_string())?;
    if !metadata.is_file() {
        return Err("icon path is not a regular file".to_string());
    }
    if metadata.len() > MAX_ICON_BYTES {
        return Err(format!("icon file too large ({} bytes)", metadata.len()));
    }

    let mut image = image::open(path).map_err(|err| err.to_string())?;
    let (width, height) = image.dimensions();
    if width > MAX_ICON_DIMENSION || height > MAX_ICON_DIMENSION {
        // Clamp oversized images to keep memory usage bounded.
        image = image.resize(
            MAX_ICON_DIMENSION,
            MAX_ICON_DIMENSION,
            FilterType::CatmullRom,
        );
    }

    let rgba = image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    if width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err("icon exceeds supported dimensions".to_string());
    }
    let width = width as i32;
    let height = height as i32;
    let stride = width.saturating_mul(4);

    Ok(RasterIcon {
        bytes: rgba.into_raw(),
        width,
        height,
        stride,
    })
}
