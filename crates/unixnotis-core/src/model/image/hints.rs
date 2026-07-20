//! Notification image hint parsing

use std::collections::HashMap;

use zbus::zvariant::{Array, OwnedValue, Structure, Value};

use crate::util;

use super::{
    ImageData, NotificationImage, MAX_ICON_NAME_BYTES, MAX_IMAGE_BYTES, MAX_IMAGE_PATH_BYTES,
};

impl NotificationImage {
    pub fn from_hints(app_name: &str, app_icon: &str, hints: &HashMap<String, OwnedValue>) -> Self {
        // The notification spec prefers image-data over image-path and app_icon
        let image_data = hints
            .get("image-data")
            .and_then(Self::parse_image_data)
            .or_else(|| hints.get("image_data").and_then(Self::parse_image_data))
            .or_else(|| hints.get("icon_data").and_then(Self::parse_image_data));
        let image_data = image_data.filter(Self::is_image_data_usable);

        let mut image_path = hints
            .get("image-path")
            .and_then(owned_to_string)
            .or_else(|| hints.get("image_path").and_then(owned_to_string))
            .map(|path| normalize_image_path(&path))
            .unwrap_or_default();

        // Desktop-entry values map to icon theme names after the suffix is removed
        let desktop_entry = hints
            .get("desktop-entry")
            .and_then(owned_to_string)
            .map(|entry| strip_desktop_suffix(&entry));
        let app_icon_path = normalize_app_icon_path(app_icon);
        if image_path.is_empty() {
            if let Some(path) = app_icon_path.as_ref() {
                image_path = path.clone();
            }
        }
        let icon_name = bound_icon_name(&resolve_icon_name(
            app_name,
            app_icon,
            app_icon_path.as_ref(),
            desktop_entry,
        ));

        Self {
            has_image_data: image_data.is_some(),
            image_data: image_data.unwrap_or_default(),
            image_path,
            icon_name,
        }
    }

    pub(super) fn parse_image_data(value: &OwnedValue) -> Option<ImageData> {
        // The image-data hint is a struct of (iiibiiay) per the notification spec
        let structure = <&Structure>::try_from(value).ok()?;
        let fields = structure.fields();
        if fields.len() != 7 {
            return None;
        }
        // Treat every field as untrusted because desktop apps do not share one strict encoder
        let width = i32::try_from(&fields[0]).ok()?;
        let height = i32::try_from(&fields[1]).ok()?;
        let rowstride = i32::try_from(&fields[2]).ok()?;
        let has_alpha = bool::try_from(&fields[3]).ok()?;
        let bits_per_sample = i32::try_from(&fields[4]).ok()?;
        let channels = i32::try_from(&fields[5]).ok()?;
        let data = Self::array_to_bytes(&fields[6])?;
        let image = ImageData {
            width,
            height,
            rowstride,
            has_alpha,
            bits_per_sample,
            channels,
            data,
        };
        Self::normalize_image_data(image)
    }

    pub(super) fn array_to_bytes(value: &Value<'_>) -> Option<Vec<u8>> {
        let array = <&Array>::try_from(value).ok()?;
        let elements = array.inner();
        // Empty payloads are not useful, and oversized payloads would waste memory downstream
        if elements.is_empty() || elements.len() > MAX_IMAGE_BYTES {
            return None;
        }
        let mut bytes = Vec::with_capacity(elements.len());
        for element in elements {
            bytes.push(u8::try_from(element).ok()?);
        }
        Some(bytes)
    }
}

fn resolve_icon_name(
    app_name: &str,
    app_icon: &str,
    app_icon_path: Option<&String>,
    desktop_entry: Option<String>,
) -> String {
    if app_icon_path.is_some() {
        return String::new();
    }
    if !app_icon.is_empty() && !app_icon.starts_with("file://") {
        return strip_desktop_suffix(app_icon);
    }
    if let Some(desktop_entry) = desktop_entry {
        return desktop_entry;
    }
    if !app_name.is_empty() {
        return app_name.to_string();
    }
    String::new()
}

fn normalize_app_icon_path(app_icon: &str) -> Option<String> {
    // Normalize the incoming icon path first so later checks operate on a cleaned,
    // bounded value rather than raw metadata input
    let path = normalize_image_path(app_icon);

    // Only accept paths that are already absolute filesystem paths or valid file URIs
    // Relative paths are rejected because app icons need to resolve unambiguously
    if path.starts_with('/') || path.starts_with("file://") {
        Some(path)
    } else {
        None
    }
}

fn normalize_image_path(value: &str) -> String {
    // Sanitize display-facing metadata and enforce the maximum byte length before
    // doing any URI-specific normalization
    let bounded = sanitize_metadata_string(value, MAX_IMAGE_PATH_BYTES);

    // File URIs get normalized into the accepted form when possible. Invalid or
    // unsupported file URI shapes fall back to an empty string
    if bounded.starts_with("file://") {
        return normalize_file_uri(&bounded).unwrap_or_default();
    }

    // Non-file URI values are returned after sanitization/truncation only
    bounded
}

fn normalize_file_uri(value: &str) -> Option<String> {
    // This function only handles file:// URIs; anything else is rejected immediately
    let stripped = value.strip_prefix("file://")?;

    // A file URI with an absolute path is already in the expected form
    if stripped.starts_with('/') {
        return Some(value.to_string());
    }

    // Convert localhost-based file URIs into the canonical absolute-path form
    stripped
        .strip_prefix("localhost/")
        .map(|path| format!("file:///{path}"))
}

fn bound_icon_name(value: &str) -> String {
    // Icon names use the same metadata sanitization path, but with the icon-name
    // byte limit instead of the image-path byte limit
    sanitize_metadata_string(value, MAX_ICON_NAME_BYTES)
}

fn sanitize_metadata_string(value: &str, max_bytes: usize) -> String {
    // Remove inline display control/problematic characters before trimming and
    // applying the final UTF-8-safe byte limit
    let cleaned = util::sanitize_inline_display_text(value);
    truncate_utf8_bytes(cleaned.trim(), max_bytes)
}

fn truncate_utf8_bytes(value: &str, max_bytes: usize) -> String {
    // Fast path: avoid allocation/truncation work when the value already fits
    if value.len() <= max_bytes {
        return value.to_string();
    }

    // Back up only across the code point that crosses the byte limit
    let mut end = max_bytes;
    for _ in 0..3 {
        if value.is_char_boundary(end) {
            break;
        }
        end -= 1;
    }
    debug_assert!(value.is_char_boundary(end));

    // Return only the byte-safe prefix
    value[..end].to_string()
}

pub(in crate::model) fn owned_to_string(value: &OwnedValue) -> Option<String> {
    // Clone the owned D-Bus value first, then attempt to extract it as a String
    // Any clone or conversion failure is represented as None
    value
        .try_clone()
        .ok()
        .and_then(|owned| String::try_from(owned).ok())
}

pub(in crate::model) fn strip_desktop_suffix(value: &str) -> String {
    // Desktop entries may include ".desktop"; icon themes usually omit it
    if let Some(stripped) = value.strip_suffix(".desktop") {
        stripped.to_string()
    } else {
        value.to_string()
    }
}
