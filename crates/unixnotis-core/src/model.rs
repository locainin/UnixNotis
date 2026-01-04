//! Notification data model and image hint parsing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zbus::zvariant::{Array, OwnedValue, Structure, Type, Value};

/// Notification urgency levels defined by the specification.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[repr(u8)]
pub enum Urgency {
    Low = 0,
    Normal = 1,
    Critical = 2,
}

impl Urgency {
    pub fn from_hint(value: Option<&OwnedValue>) -> Self {
        let Some(value) = value else {
            return Self::Normal;
        };
        let level = if let Ok(v) = u8::try_from(value) {
            v as u32
        } else if let Ok(v) = u32::try_from(value) {
            v
        } else {
            return Self::Normal;
        };

        match level {
            0 => Self::Low,
            2 => Self::Critical,
            _ => Self::Normal,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Action pair in the notification protocol.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Action {
    pub key: String,
    pub label: String,
}

/// Raw image data payload from hints.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct ImageData {
    pub width: i32,
    pub height: i32,
    pub rowstride: i32,
    pub has_alpha: bool,
    pub bits_per_sample: i32,
    pub channels: i32,
    pub data: Vec<u8>,
}

/// Image information derived from standard hints and app_icon.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct NotificationImage {
    pub has_image_data: bool,
    pub image_data: ImageData,
    pub image_path: String,
    pub icon_name: String,
}

const MAX_IMAGE_BYTES: usize = 1024 * 1024;
const MAX_IMAGE_DIMENSION: i32 = 512;

/// Full notification record stored by the daemon.
#[derive(Debug)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<Action>,
    pub hints: HashMap<String, OwnedValue>,
    pub urgency: Urgency,
    pub category: Option<String>,
    pub is_transient: bool,
    pub is_resident: bool,
    pub image: NotificationImage,
    pub expire_timeout: i32,
    pub received_at: DateTime<Utc>,
}

impl Notification {
    /// Convert to a lightweight view for UI consumption.
    pub fn to_view(&self) -> NotificationView {
        NotificationView {
            id: self.id,
            app_name: self.app_name.clone(),
            summary: self.summary.clone(),
            body: self.body.clone(),
            actions: self.actions.clone(),
            urgency: self.urgency.as_u8(),
            is_transient: self.is_transient,
            is_resident: self.is_resident,
            received_at_unix_ms: self.received_at.timestamp_millis(),
            image: self.image.clone(),
        }
    }

    pub fn to_list_view(&self) -> NotificationView {
        NotificationView {
            id: self.id,
            app_name: self.app_name.clone(),
            summary: self.summary.clone(),
            body: self.body.clone(),
            actions: self.actions.clone(),
            urgency: self.urgency.as_u8(),
            is_transient: self.is_transient,
            is_resident: self.is_resident,
            received_at_unix_ms: self.received_at.timestamp_millis(),
            image: self.image.for_listing(),
        }
    }

    pub fn to_history(&self) -> Notification {
        Notification {
            id: self.id,
            app_name: self.app_name.clone(),
            app_icon: self.app_icon.clone(),
            summary: self.summary.clone(),
            body: self.body.clone(),
            actions: self.actions.clone(),
            hints: HashMap::new(),
            urgency: self.urgency,
            category: self.category.clone(),
            is_transient: self.is_transient,
            is_resident: self.is_resident,
            image: self.image.for_history(),
            expire_timeout: self.expire_timeout,
            received_at: self.received_at,
        }
    }
}

/// Serializable view of a notification for D-Bus signals.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NotificationView {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<Action>,
    pub urgency: u8,
    pub is_transient: bool,
    pub is_resident: bool,
    pub received_at_unix_ms: i64,
    pub image: NotificationImage,
}

impl NotificationImage {
    pub fn from_hints(app_name: &str, app_icon: &str, hints: &HashMap<String, OwnedValue>) -> Self {
        // The spec prefers image-data over image-path and app_icon.
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

        // Normalize desktop-entry values to icon theme names by stripping ".desktop".
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
        let icon_name = if app_icon_path.is_some() {
            String::new()
        } else if !app_icon.is_empty() {
            strip_desktop_suffix(app_icon)
        } else if let Some(desktop_entry) = desktop_entry {
            desktop_entry
        } else if !app_name.is_empty() {
            app_name.to_string()
        } else {
            String::new()
        };

        Self {
            has_image_data: image_data.is_some(),
            image_data: image_data.unwrap_or_default(),
            image_path,
            icon_name,
        }
    }

    fn for_listing(&self) -> Self {
        if self.image_data.data.is_empty() {
            return self.clone();
        }
        Self {
            has_image_data: false,
            image_data: ImageData::default(),
            image_path: self.image_path.clone(),
            icon_name: self.icon_name.clone(),
        }
    }

    fn is_image_data_usable(data: &ImageData) -> bool {
        if data.width <= 0 || data.height <= 0 {
            return false;
        }
        if data.width > MAX_IMAGE_DIMENSION || data.height > MAX_IMAGE_DIMENSION {
            return false;
        }
        data.data.len() <= MAX_IMAGE_BYTES
    }

    fn parse_image_data(value: &OwnedValue) -> Option<ImageData> {
        // The image-data hint is a struct of (iiibiiay) per the spec.
        let structure = <&Structure>::try_from(value).ok()?;
        let fields = structure.fields();
        if fields.len() != 7 {
            return None;
        }
        let width = i32::try_from(&fields[0]).ok()?;
        let height = i32::try_from(&fields[1]).ok()?;
        let rowstride = i32::try_from(&fields[2]).ok()?;
        let has_alpha = bool::try_from(&fields[3]).ok()?;
        let bits_per_sample = i32::try_from(&fields[4]).ok()?;
        let channels = i32::try_from(&fields[5]).ok()?;

        if bits_per_sample == 8 && channels == 3 {
            return Self::expand_rgb_array_to_rgba(
                width,
                height,
                rowstride,
                bits_per_sample,
                &fields[6],
            );
        }

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

    pub fn for_history(&self) -> NotificationImage {
        if self.has_image_data
            && (!self.image_path.is_empty() || !self.icon_name.is_empty())
        {
            let mut trimmed = self.clone();
            trimmed.has_image_data = false;
            trimmed.image_data = ImageData::default();
            return trimmed;
        }
        self.clone()
    }

    fn normalize_image_data(image: ImageData) -> Option<ImageData> {
        if image.bits_per_sample != 8 {
            return Some(image);
        }
        match image.channels {
            4 => Some(image),
            3 => Self::expand_rgb_to_rgba(&image),
            _ => None,
        }
    }

    fn array_to_bytes(value: &Value<'_>) -> Option<Vec<u8>> {
        let array = <&Array>::try_from(value).ok()?;
        let elements = array.inner();
        let mut bytes = Vec::with_capacity(elements.len());
        for element in elements {
            bytes.push(u8::try_from(element).ok()?);
        }
        Some(bytes)
    }

    fn expand_rgb_array_to_rgba(
        width: i32,
        height: i32,
        rowstride: i32,
        bits_per_sample: i32,
        data_value: &Value<'_>,
    ) -> Option<ImageData> {
        let array = <&Array>::try_from(data_value).ok()?;
        let elements = array.inner();
        let width_px = width.max(1) as usize;
        let height_px = height.max(1) as usize;
        let rowstride_bytes = if rowstride > 0 {
            rowstride as usize
        } else {
            width_px * 3
        };
        let mut rgba = vec![0u8; width_px * height_px * 4];

        for y in 0..height_px {
            let row_start = y.saturating_mul(rowstride_bytes);
            for x in 0..width_px {
                let idx = row_start.saturating_add(x * 3);
                if idx + 2 >= elements.len() {
                    return None;
                }
                let dst = (y * width_px + x) * 4;
                rgba[dst] = u8::try_from(&elements[idx]).ok()?;
                rgba[dst + 1] = u8::try_from(&elements[idx + 1]).ok()?;
                rgba[dst + 2] = u8::try_from(&elements[idx + 2]).ok()?;
                rgba[dst + 3] = 255;
            }
        }

        Some(ImageData {
            width,
            height,
            rowstride: (width_px * 4) as i32,
            has_alpha: true,
            bits_per_sample,
            channels: 4,
            data: rgba,
        })
    }

    fn expand_rgb_to_rgba(image: &ImageData) -> Option<ImageData> {
        let width = image.width.max(1) as usize;
        let height = image.height.max(1) as usize;
        let rowstride = if image.rowstride > 0 {
            image.rowstride as usize
        } else {
            width * 3
        };
        let mut rgba = vec![0u8; width * height * 4];

        for y in 0..height {
            let row_start = y.saturating_mul(rowstride);
            for x in 0..width {
                let idx = row_start.saturating_add(x * 3);
                if idx + 2 >= image.data.len() {
                    return None;
                }
                let dst = (y * width + x) * 4;
                rgba[dst] = image.data[idx];
                rgba[dst + 1] = image.data[idx + 1];
                rgba[dst + 2] = image.data[idx + 2];
                rgba[dst + 3] = 255;
            }
        }

        Some(ImageData {
            width: image.width,
            height: image.height,
            rowstride: (width * 4) as i32,
            has_alpha: true,
            bits_per_sample: image.bits_per_sample,
            channels: 4,
            data: rgba,
        })
    }
}

fn owned_to_string(value: &OwnedValue) -> Option<String> {
    value
        .try_clone()
        .ok()
        .and_then(|owned| String::try_from(owned).ok())
}

fn strip_desktop_suffix(value: &str) -> String {
    // Desktop entries may include ".desktop"; icon themes typically omit the suffix.
    if let Some(stripped) = value.strip_suffix(".desktop") {
        stripped.to_string()
    } else {
        value.to_string()
    }
}
