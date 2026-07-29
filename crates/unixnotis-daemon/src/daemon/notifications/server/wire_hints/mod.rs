//! Bounded deserialization for caller-provided notification hints

mod decode;
mod image_bytes;

use std::collections::HashMap;

use unixnotis_core::ImageData;
use zbus::zvariant::{OwnedValue, Signature, Type};

/// Hints decoded without expanding large byte arrays into per-byte dynamic values
#[derive(Debug, Default)]
pub(super) struct WireHints {
    values: HashMap<String, OwnedValue>,
    image_data: Option<ImageData>,
}

impl WireHints {
    pub(super) fn into_parts(self) -> (HashMap<String, OwnedValue>, Option<ImageData>) {
        (self.values, self.image_data)
    }
}

impl From<HashMap<String, OwnedValue>> for WireHints {
    fn from(values: HashMap<String, OwnedValue>) -> Self {
        // Internal tests and helpers may still supply an already-decoded hint map
        Self {
            values,
            image_data: None,
        }
    }
}

impl Type for WireHints {
    fn signature() -> Signature<'static> {
        // This is the standard freedesktop notification hint dictionary
        Signature::from_static_str_unchecked("a{sv}")
    }
}
