//! Icon source discovery for notifications.
//!
//! Groups desktop icon lookup, themed icon resolution, and image decoding helpers.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use gio::prelude::{AppInfoExt, FileExt};
use gtk::gdk;
use gtk::gdk::prelude::*;
use gtk::{IconLookupFlags, IconPaintable, TextDirection};
use unixnotis_core::{NotificationImage, NotificationView};

use super::icons_cache::CachedPaintable;

pub(super) enum IconSource {
    Paintable(IconPaintable),
    RasterPath(PathBuf),
}

pub(super) fn resolve_icon_source(name: &str, size: i32, scale: i32) -> Option<IconSource> {
    // Resolve a themed icon into a GTK paintable at the requested size/scale.
    // If the paintable originates from a non-SVG file on disk, we prefer returning the path
    // so the raster decode pipeline can cache + decode off-thread (avoids main-thread spikes).
    let paintable = resolve_icon_paintable(name, size, scale)?;

    // Some paintables are backed by a gio::File (theme icons loaded from disk). If we can get a real
    // filesystem path and it's not SVG, treat it as a raster path source.
    if let Some(file) = paintable.file() {
        if let Some(path) = file.path() {
            // SVG decoding/rendering often stays on the GTK side; only fast-path raster files here.
            if !is_svg_path(&path) {
                return Some(IconSource::RasterPath(path));
            }
        }
    }

    // Fallback: keep the paintable (covers SVGs, non-file paintables, and theme backends).
    Some(IconSource::Paintable(paintable))
}

pub(super) fn file_path_from_hint(path: &str) -> Option<&Path> {
    // Notification hints may provide a direct absolute path or a file:// URL.
    // We normalize both into a Path, and reject anything else (http, relative, etc.).
    if path.starts_with('/') {
        return Some(Path::new(path));
    }
    if let Some(stripped) = path.strip_prefix("file://") {
        return Some(Path::new(stripped));
    }
    None
}

pub(super) fn resolve_path_texture(path: &Path) -> Option<CachedPaintable> {
    // Only load real files from disk; avoids weird behavior for directories/symlinks/invalid paths.
    if !path.is_file() {
        return None;
    }

    // Let GDK load the texture directly. This is a synchronous path and is typically fine for small icons;
    // heavy/large loads should prefer the async raster decode pipeline when possible.
    let file = gio::File::for_path(path);
    let texture = gdk::Texture::from_file(&file).ok()?;
    Some(CachedPaintable::from_texture(texture))
}

pub(super) fn is_svg_path(path: &Path) -> bool {
    // SVG/SVGZ should stay on GTK's paintable path (scaling/vector rendering rules differ from raster).
    // Case-insensitive check on extension.
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "svg" | "svgz"))
        .unwrap_or(false)
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
    // Normalizes keys for consistent map lookups / comparisons:
    // - trim removes accidental whitespace
    // - lowercase makes lookups case-insensitive (theme/icon names often vary in casing)
    value.trim().to_lowercase()
}

fn is_missing_icon(path: &Path) -> bool {
    // Ignore theme placeholders to avoid rendering missing-icon glyphs.
    // Many icon themes provide an "image-missing" asset; treating it as a real icon looks bad.
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false; // Non-UTF8 or missing filename stem; don't classify as missing placeholder.
    };
    stem.starts_with("image-missing")
}

pub(super) fn image_data_texture(image: &NotificationImage) -> Option<gdk::Texture> {
    // Only proceed if the notification actually carried image-data (not just a name/path hint).
    if !image.has_image_data {
        return None;
    }

    let data = &image.image_data;

    // The standard image-data payload for notifications is typically 8 bits per channel.
    // If it's not 8, we don't currently support it (avoids misinterpreting the byte layout).
    if data.bits_per_sample != 8 {
        return None;
    }

    // Clamp dimensions so we never pass 0 to MemoryTexture (which would error or behave oddly).
    let width = data.width.max(1) as u32;
    let height = data.height.max(1) as u32;

    // We only handle RGBA (channels == 4) here because MemoryFormat::R8g8b8a8 expects 4 bytes/pixel.
    // If the payload is RGB or something else, it should have been expanded earlier in the pipeline.
    let bytes = if data.channels == 4 {
        gtk::glib::Bytes::from(&data.data)
    } else {
        return None;
    };

    // Rowstride is bytes per row; Hypr/notify image-data can include padding.
    // If rowstride is invalid/zero, fall back to tightly packed RGBA (width * 4).
    let stride = if data.rowstride > 0 {
        data.rowstride as usize
    } else {
        (width * 4) as usize
    };

    // Build a GPU texture from the raw pixel bytes. MemoryFormat must match the byte layout.
    Some(
        gdk::MemoryTexture::new(
            width as i32,
            height as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            stride,
        )
        .upcast::<gdk::Texture>(),
    )
}
