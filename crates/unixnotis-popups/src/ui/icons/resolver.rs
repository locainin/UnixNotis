//! Icon resolution and image helpers for popup rendering.
//!
//! Separates icon lookup and image decoding from UI state management.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gio::prelude::FileExt;
use gtk::gdk;
use gtk::{IconLookupFlags, IconPaintable, TextDirection};
use unixnotis_core::NotificationView;

pub(in crate::ui) fn file_path_from_hint(path: &str) -> Option<PathBuf> {
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

pub(in crate::ui) fn resolve_icon_image(name: &str, size: i32) -> Option<gtk::Image> {
    // File-path icons are resolved asynchronously in the UI layer to avoid blocking the GTK thread.
    let paintable = resolve_icon_paintable(name, size)?;
    let widget = gtk::Image::from_paintable(Some(&paintable));
    widget.set_pixel_size(size);
    Some(widget)
}

pub(in crate::ui) fn collect_icon_candidates(notification: &NotificationView) -> Vec<String> {
    let mut candidates = Vec::new();
    if !notification.attribution.badge_icon.is_empty() {
        candidates.push(notification.attribution.badge_icon.clone());
        if let Some(stripped) = notification.attribution.badge_icon.strip_suffix(".desktop") {
            candidates.push(stripped.to_string());
        }
        candidates.push(notification.attribution.badge_icon.to_lowercase());
    }
    let authenticated_primary =
        notification.attribution.verified || !notification.attribution.reported_name.is_empty();
    if authenticated_primary && !notification.app_name.is_empty() {
        // Unresolved claims never become badge candidates when the warning icon is unavailable
        candidates.push(notification.app_name.clone());
        let lower = notification.app_name.to_lowercase();
        let dashed = lower.replace(' ', "-");
        candidates.push(lower);
        candidates.push(dashed);
    }

    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| !candidate.is_empty() && seen.insert(candidate.clone()))
        .collect()
}

fn is_missing_icon(path: &Path) -> bool {
    // Filter the theme placeholder to avoid rendering a missing-icon glyph.
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    stem.starts_with("image-missing")
}

#[cfg(test)]
#[path = "tests/resolver/mod.rs"]
mod tests;
