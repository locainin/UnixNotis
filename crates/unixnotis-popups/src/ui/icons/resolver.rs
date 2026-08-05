//! Icon resolution and image helpers for popup rendering.
//!
//! Separates icon lookup and image decoding from UI state management.

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
pub(in crate::ui) fn resolve_icon_paintable_with_scale(
    name: &str,
    size: i32,
    scale: i32,
) -> Option<IconPaintable> {
    if name.is_empty() {
        return None;
    }
    let display = gdk::Display::default()?;
    let icon_theme = gtk::IconTheme::for_display(&display);
    let paintable = icon_theme.lookup_icon(
        name,
        &[],
        size,
        scale.max(1),
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

pub(in crate::ui) fn collect_icon_candidates(notification: &NotificationView) -> Vec<String> {
    // Candidate lists stay small, so ordered linear deduplication avoids a hash allocation
    let mut candidates = Vec::with_capacity(7);
    let badge_icon = notification.attribution.badge_icon.as_str();
    if !badge_icon.is_empty() {
        push_candidate(&mut candidates, badge_icon);
        if let Some(stripped) = badge_icon.strip_suffix(".desktop") {
            push_candidate(&mut candidates, stripped);
        }
        let lowercase = badge_icon.to_lowercase();
        push_candidate(&mut candidates, &lowercase);
    }
    let desktop_id = notification.attribution.desktop_id.as_str();
    if !desktop_id.is_empty() {
        // Desktop ids are daemon-associated metadata and safe badge lookup candidates
        push_candidate(&mut candidates, desktop_id);
        let lowercase = desktop_id.to_lowercase();
        push_candidate(&mut candidates, &lowercase);
    }
    let claimed_theme_icon = notification.image.claimed_theme_icon.as_str();
    if is_safe_theme_name(claimed_theme_icon) {
        // Sender input is only a bounded theme lookup hint, never identity evidence
        push_candidate(&mut candidates, claimed_theme_icon);
        let lowercase = claimed_theme_icon.to_lowercase();
        push_candidate(&mut candidates, &lowercase);
    }
    candidates
}

fn push_candidate(candidates: &mut Vec<String>, candidate: &str) {
    if candidate.is_empty() || candidates.iter().any(|existing| existing == candidate) {
        return;
    }
    candidates.push(candidate.to_owned());
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
