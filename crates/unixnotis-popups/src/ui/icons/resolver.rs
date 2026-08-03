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
    if !notification.attribution.desktop_id.is_empty() {
        // Desktop ids are daemon-associated metadata and safe badge lookup candidates
        candidates.push(notification.attribution.desktop_id.clone());
        candidates.push(notification.attribution.desktop_id.to_lowercase());
    }
    if is_safe_theme_name(&notification.image.claimed_theme_icon) {
        // Sender input is only a bounded theme lookup hint, never identity evidence
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
    // Filter the theme placeholder to avoid rendering a missing-icon glyph.
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    stem.starts_with("image-missing")
}

#[cfg(test)]
#[path = "tests/resolver/mod.rs"]
mod tests;
