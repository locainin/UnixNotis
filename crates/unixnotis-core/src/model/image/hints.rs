//! Notification image hint parsing

use std::collections::HashMap;

use zbus::zvariant::{Array, OwnedValue, Structure, Value};

use super::{ImageData, NotificationImage, MAX_IMAGE_BYTES};

impl NotificationImage {
    pub fn from_hints(
        _app_name: &str,
        _app_icon: &str,
        hints: &HashMap<String, OwnedValue>,
    ) -> Self {
        // Embedded pixels are already detached from the sender's filesystem
        let image_data = hints
            .get("image-data")
            .and_then(Self::parse_image_data)
            .or_else(|| hints.get("image_data").and_then(Self::parse_image_data))
            .or_else(|| hints.get("icon_data").and_then(Self::parse_image_data));
        let image_data = image_data.filter(Self::is_image_data_usable);

        Self {
            badge_icon: String::new(),
            sender_visual_role: super::NotificationVisualRole::None,
            sender_visual: ImageData::default(),
            content_image: image_data.unwrap_or_default(),
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
