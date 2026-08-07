//! Key-aware variant decoding for the freedesktop notification hint map

use std::collections::HashMap;

use serde::de::{DeserializeSeed, Deserializer, Error as _, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use zbus::zvariant::{OwnedValue, Signature, Value};

use super::image_bytes::{BoundedImageBytes, WireImageData};
use super::WireHints;

impl<'de> Deserialize<'de> for WireHints {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(WireHintsVisitor)
    }
}

struct WireHintsVisitor;

impl<'de> Visitor<'de> for WireHintsVisitor {
    type Value = WireHints;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a freedesktop notification hint dictionary")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = HashMap::with_capacity(map.size_hint().unwrap_or_default());
        let mut standard_image = None;
        let mut legacy_image = None;
        let mut legacy_icon = None;
        let mut image_path = None;

        while let Some(key) = map.next_key::<String>()? {
            let Some(kind) = HintKind::for_key(&key) else {
                // Raw preflight bounds unknown values before this owned fallback runs
                map.next_value::<OwnedValue>()?;
                continue;
            };
            let decoded = map.next_value_seed(HintVariantSeed { kind })?;
            match decoded {
                DecodedHint::Text(text) => {
                    let value = owned_string(&text).map_err(A::Error::custom)?;
                    if matches!(key.as_str(), "image-path" | "image_path") {
                        image_path = value
                            .try_clone()
                            .ok()
                            .and_then(|owned| String::try_from(owned).ok());
                    }
                    values.insert(key, value);
                }
                DecodedHint::Bool(value) => {
                    values.insert(key, OwnedValue::from(value));
                }
                DecodedHint::Urgency(value) => {
                    values.insert(key, OwnedValue::from(value));
                }
                DecodedHint::Image(Some(image)) => {
                    // Keep each protocol alias separate so arrival order cannot change precedence
                    match key.as_str() {
                        "image-data" => standard_image = Some(image),
                        "image_data" => legacy_image = Some(image),
                        "icon_data" => legacy_icon = Some(image),
                        _ => {}
                    }
                }
                DecodedHint::Image(None) => {}
            }
        }

        Ok(WireHints {
            values,
            wire_image_data: standard_image.or(legacy_image).or(legacy_icon),
            image_path,
        })
    }
}

#[derive(Clone, Copy)]
enum HintKind {
    Text,
    Bool,
    Urgency,
    Image,
}

impl HintKind {
    fn for_key(key: &str) -> Option<Self> {
        match key {
            "desktop-entry"
            | "category"
            | "image-path"
            | "image_path"
            | "sound-name"
            | "sound-file"
            | "x-kde-reply-placeholder-text"
            | "x-kde-reply-submit-button-text"
            | "x-kde-reply-submit-button-icon-name" => Some(Self::Text),
            "transient" | "resident" | "suppress-sound" => Some(Self::Bool),
            "urgency" => Some(Self::Urgency),
            "image-data" | "image_data" | "icon_data" => Some(Self::Image),
            _ => None,
        }
    }
}

enum DecodedHint {
    Text(String),
    Bool(bool),
    Urgency(u32),
    Image(Option<WireImageData>),
}

struct HintVariantSeed {
    kind: HintKind,
}

impl<'de> DeserializeSeed<'de> for HintVariantSeed {
    type Value = DecodedHint;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(HintVariantVisitor { kind: self.kind })
    }
}

struct HintVariantVisitor {
    kind: HintKind,
}

impl<'de> Visitor<'de> for HintVariantVisitor {
    type Value = DecodedHint;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a typed notification hint variant")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let signature = sequence
            .next_element::<Signature<'_>>()?
            .ok_or_else(|| A::Error::invalid_length(0, &self))?;
        match self.kind {
            HintKind::Text if signature.as_str() == "s" => {
                sequence.next_element::<String>()?.map_or_else(
                    || Err(A::Error::invalid_length(1, &self)),
                    |value| Ok(DecodedHint::Text(value)),
                )
            }
            HintKind::Bool if signature.as_str() == "b" => {
                sequence.next_element::<bool>()?.map_or_else(
                    || Err(A::Error::invalid_length(1, &self)),
                    |value| Ok(DecodedHint::Bool(value)),
                )
            }
            HintKind::Urgency if signature.as_str() == "y" => {
                sequence.next_element::<u8>()?.map_or_else(
                    || Err(A::Error::invalid_length(1, &self)),
                    |value| Ok(DecodedHint::Urgency(u32::from(value))),
                )
            }
            HintKind::Urgency if signature.as_str() == "u" => {
                sequence.next_element::<u32>()?.map_or_else(
                    || Err(A::Error::invalid_length(1, &self)),
                    |value| Ok(DecodedHint::Urgency(value)),
                )
            }
            HintKind::Image if signature.as_str() == "(iiibiiay)" => {
                let raw = sequence
                    .next_element::<(i32, i32, i32, bool, i32, i32, BoundedImageBytes)>()?
                    .ok_or_else(|| A::Error::invalid_length(1, &self))?;
                let image = raw
                    .6
                    .into_wire_image(raw.0, raw.1, raw.2, raw.3, raw.4, raw.5);
                Ok(DecodedHint::Image(image))
            }
            HintKind::Text | HintKind::Bool | HintKind::Urgency | HintKind::Image => Err(
                A::Error::custom("notification hint has an unexpected D-Bus signature"),
            ),
        }
    }
}

fn owned_string(value: &str) -> zbus::zvariant::Result<OwnedValue> {
    OwnedValue::try_from(Value::from(value))
}
