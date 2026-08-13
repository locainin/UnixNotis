//! Icon resolution and image helpers for popup rendering.
//!
//! Separates icon lookup and image decoding from UI state management.

use std::path::{Path, PathBuf};

use gio::prelude::FileExt;
use gtk::gdk;
use gtk::{IconLookupFlags, IconPaintable, TextDirection};
use unixnotis_core::{AttributionStatus, NotificationView};

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
    let mut candidates = Vec::with_capacity(12);
    if notification.attribution.status == AttributionStatus::Unresolved {
        push_claimed_icon_candidates(&mut candidates, notification);
        push_attributed_icon_candidates(&mut candidates, notification);
    } else {
        push_attributed_icon_candidates(&mut candidates, notification);
        push_claimed_icon_candidates(&mut candidates, notification);
    }
    candidates
}

fn push_attributed_icon_candidates(candidates: &mut Vec<String>, notification: &NotificationView) {
    let badge_icon = notification.attribution.badge_icon.as_str();
    if !badge_icon.is_empty() {
        push_candidate(candidates, badge_icon);
        if let Some(stripped) = badge_icon.strip_suffix(".desktop") {
            push_candidate(candidates, stripped);
        }
        let lowercase = badge_icon.to_lowercase();
        push_candidate(candidates, &lowercase);
    }

    let desktop_id = notification.attribution.desktop_id.as_str();
    if !desktop_id.is_empty() {
        push_candidate(candidates, desktop_id);
        if let Some(stripped) = desktop_id.strip_suffix(".desktop") {
            push_candidate(candidates, stripped);
        }
        let lowercase = desktop_id.to_lowercase();
        push_candidate(candidates, &lowercase);
    }
}

fn push_claimed_icon_candidates(candidates: &mut Vec<String>, notification: &NotificationView) {
    // Claimed names only select presentation candidates; they never prove identity
    let claimed_desktop_id = notification.image.claimed_desktop_id.as_str();
    if is_safe_theme_name(claimed_desktop_id) {
        push_candidate(candidates, claimed_desktop_id);
        if let Some(stripped) = claimed_desktop_id.strip_suffix(".desktop") {
            push_candidate(candidates, stripped);
        }
        let lowercase = claimed_desktop_id.to_lowercase();
        push_candidate(candidates, &lowercase);
    }

    let claimed_theme_icon = notification.image.claimed_theme_icon.as_str();
    if is_safe_theme_name(claimed_theme_icon) {
        push_candidate(candidates, claimed_theme_icon);
        let lowercase = claimed_theme_icon.to_lowercase();
        push_candidate(candidates, &lowercase);
    }
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
