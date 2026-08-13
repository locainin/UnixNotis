//! Bounded deserialization for caller-provided notification hints

mod decode;
mod image_bytes;

use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Signature, Type};

pub(super) use self::image_bytes::WireImageData;

/// Hints decoded without expanding large byte arrays into per-byte dynamic values
#[derive(Debug, Default)]
pub(super) struct WireHints {
    values: HashMap<String, OwnedValue>,
    wire_image_data: Option<WireImageData>,
    image_path: Option<String>,
}

impl WireHints {
    pub(super) fn into_parts(
        self,
    ) -> (
        HashMap<String, OwnedValue>,
        Option<WireImageData>,
        Option<String>,
    ) {
        (self.values, self.wire_image_data, self.image_path)
    }
}

impl From<HashMap<String, OwnedValue>> for WireHints {
    fn from(values: HashMap<String, OwnedValue>) -> Self {
        // Internal tests and helpers may still supply an already-decoded hint map
        let image_path = values
            .get("image-path")
            .or_else(|| values.get("image_path"))
            .and_then(|value| value.try_clone().ok())
            .and_then(|value| String::try_from(value).ok());
        Self {
            values,
            wire_image_data: None,
            image_path,
        }
    }
}

impl Type for WireHints {
    fn signature() -> Signature<'static> {
        // This is the standard freedesktop notification hint dictionary
        Signature::from_static_str_unchecked("a{sv}")
    }
}
