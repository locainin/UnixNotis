//! Notification image hint parsing

use std::collections::HashMap;

use zbus::zvariant::{Array, OwnedValue, Structure, Value};

use super::{ImageData, NotificationImage, MAX_IMAGE_BYTES};

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
            .unwrap_or_default();

        // Desktop-entry values map to icon theme names after the suffix is removed
        let desktop_entry = hints
            .get("desktop-entry")
            .and_then(owned_to_string)
            .map(|entry| strip_desktop_suffix(&entry));
        let app_icon_path = if app_icon.starts_with('/') || app_icon.starts_with("file://") {
            Some(app_icon.to_string())
        } else {
            None
        };
        if image_path.is_empty() {
            if let Some(path) = app_icon_path.as_ref() {
                image_path = path.clone();
            }
        }
        let icon_name =
            resolve_icon_name(app_name, app_icon, app_icon_path.as_ref(), desktop_entry);

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
    if !app_icon.is_empty() {
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

pub(super) fn owned_to_string(value: &OwnedValue) -> Option<String> {
    value
        .try_clone()
        .ok()
        .and_then(|owned| String::try_from(owned).ok())
}

pub(super) fn strip_desktop_suffix(value: &str) -> String {
    // Desktop entries may include ".desktop"; icon themes usually omit it
    if let Some(stripped) = value.strip_suffix(".desktop") {
        stripped.to_string()
    } else {
        value.to_string()
    }
}
