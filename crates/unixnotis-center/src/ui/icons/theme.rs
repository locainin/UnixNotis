//! Icon source discovery for notifications
//!
//! Groups desktop icon lookup, themed icon resolution, and image decoding helpers

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gio::prelude::FileExt;
use gtk::gdk;
use gtk::prelude::*;
use gtk::{IconLookupFlags, IconPaintable, TextDirection};
use unixnotis_core::{ImageData, NotificationImage, NotificationView};

pub(super) enum IconSource {
    Paintable(IconPaintable),
    RasterPath(PathBuf),
}

pub(super) fn resolve_icon_source(name: &str, size: i32, scale: i32) -> Option<IconSource> {
    // Resolve a themed icon into a GTK paintable at the requested size/scale
    // If the paintable originates from a non-SVG file on disk, we prefer returning the path
    // so the raster decode pipeline can cache + decode off-thread (avoids main-thread spikes)
    let paintable = resolve_icon_paintable(name, size, scale)?;

    // Some paintables are backed by a gio::File (theme icons loaded from disk). If we can get a real
    // filesystem path and it's not SVG, treat it as a raster path source
    if let Some(file) = paintable.file() {
        if let Some(path) = file.path() {
            // Only formats handled by the bounded worker leave GTK's theme paintable path
            if theme_path_uses_worker(&path) {
                return Some(IconSource::RasterPath(path));
            }
        }
    }

    // Fallback: keep the paintable (covers SVGs, non-file paintables, and theme backends)
    Some(IconSource::Paintable(paintable))
}

fn worker_decodes_theme_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tif" | "tiff" | "webp" | "ico"
            )
        })
}

fn theme_path_uses_worker(path: &Path) -> bool {
    // Theme links stay with GTK while regular raster files use the bounded worker
    worker_decodes_theme_path(path) && !path.is_symlink()
}

fn resolve_icon_paintable(name: &str, size: i32, scale: i32) -> Option<IconPaintable> {
    if name.is_empty() {
        return None;
    }
    let display = gdk::Display::default()?;
    let icon_theme = gtk::IconTheme::for_display(&display);
    let paintable = icon_theme.lookup_icon(
        name,
        &[],
        size,
        scale,
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

pub(super) fn collect_icon_candidates(notification: &NotificationView) -> Vec<String> {
    let mut candidates = Vec::new();
    if !notification.attribution.badge_icon.is_empty() {
        candidates.push(notification.attribution.badge_icon.clone());
        if let Some(stripped) = notification.attribution.badge_icon.strip_suffix(".desktop") {
            candidates.push(stripped.to_string());
        }
        candidates.push(notification.attribution.badge_icon.to_lowercase());
    }
    if !notification.attribution.desktop_id.is_empty() {
        // Desktop ids are daemon-associated metadata and safe badge lookup candidates
        candidates.push(notification.attribution.desktop_id.clone());
        candidates.push(notification.attribution.desktop_id.to_lowercase());
    }
    if is_safe_theme_name(&notification.image.claimed_theme_icon) {
        // The daemon has bounded this value and rejected path-like input
        candidates.push(notification.image.claimed_theme_icon.clone());
        candidates.push(notification.image.claimed_theme_icon.to_lowercase());
    }
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| !candidate.is_empty() && seen.insert(candidate.clone()))
        .collect()
}

fn is_safe_theme_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('.')
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains(':')
        && !value.chars().any(char::is_whitespace)
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

fn is_missing_icon(path: &Path) -> bool {
    // Ignore theme placeholders to avoid rendering missing-icon glyphs
    // Many icon themes provide an "image-missing" asset; treating it as a real icon looks bad
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false; // Non-UTF8 or missing filename stem; don't classify as missing placeholder.
    };
    stem.starts_with("image-missing")
}

pub(super) fn image_data_texture(image: &NotificationImage) -> Option<gdk::Texture> {
    // Only proceed if the notification actually carried image-data (not just a name/path hint)
    if image.content_image.data.is_empty() {
        return None;
    }

    image_data_texture_for_data(&image.content_image)
}

pub(super) fn image_data_texture_for_data(data: &ImageData) -> Option<gdk::Texture> {
    // The standard image-data payload for notifications is typically 8 bits per channel
    // If it's not 8, the byte layout is ambiguous for this path, so reject it
    if data.bits_per_sample != 8 {
        return None;
    }
    // Negative rowstride is invalid for pixel buffers
    if data.rowstride < 0 {
        return None;
    }

    // Reject non-positive dimensions before creating the texture
    if data.width <= 0 || data.height <= 0 {
        return None;
    }
    let width = data.width as usize;
    let height = data.height as usize;
    let width_i32 = i32::try_from(width).ok()?;
    let height_i32 = i32::try_from(height).ok()?;

    // Select a conversion path based on the channel layout
    // RGBA can be used directly, RGB is expanded to RGBA with an opaque alpha
    let (bytes, stride) = match data.channels {
        4 => {
            // Rowstride is bytes per row; hint payloads may include padding
            // If rowstride is invalid/zero, fall back to tightly packed RGBA (width * 4)
            let min_stride = width.checked_mul(4)?;
            let stride = if data.rowstride > 0 {
                data.rowstride as usize
            } else {
                min_stride
            };
            // Validate rowstride and buffer length before building the texture
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
            // RGB payloads are valid per spec; expand to RGBA with alpha=255
            let (expanded, stride) = expand_rgb_to_rgba(data)?;
            (gtk::glib::Bytes::from(&expanded), stride)
        }
        _ => {
            // Other channel counts are not supported by the RGBA texture path
            return None;
        }
    };

    // Build a GPU texture from the raw pixel bytes. MemoryFormat must match the byte layout
    Some(
        gdk::MemoryTexture::new(
            width_i32,
            height_i32,
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            stride,
        )
        .upcast::<gdk::Texture>(),
    )
}

fn expand_rgb_to_rgba(data: &unixnotis_core::ImageData) -> Option<(Vec<u8>, usize)> {
    // Expand RGB to RGBA while honoring per-row padding in the source buffer
    let width = usize::try_from(data.width).ok()?;
    let height = usize::try_from(data.height).ok()?;
    if width == 0 || height == 0 {
        return None;
    }

    // Source stride handles optional per-row padding for RGB input
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

    // Destination uses tightly packed RGBA rows
    let dst_stride = width.checked_mul(4)?;
    let mut rgba = vec![0u8; dst_stride.checked_mul(height)?];

    // Copy RGB per pixel and append opaque alpha
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
#[path = "tests/theme.rs"]
mod tests;
