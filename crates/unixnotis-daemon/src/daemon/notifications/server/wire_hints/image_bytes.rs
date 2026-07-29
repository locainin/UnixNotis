//! Allocation-bounded byte-array decoding for optional notification images

use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use unixnotis_core::ImageData;

/// Raw images larger than the retained model limit are consumed but never allocated
#[derive(Debug, Default)]
pub(super) struct BoundedImageBytes {
    data: Option<Vec<u8>>,
}

impl BoundedImageBytes {
    pub(super) fn into_image_data(
        self,
        width: i32,
        height: i32,
        rowstride: i32,
        has_alpha: bool,
        bits_per_sample: i32,
        channels: i32,
    ) -> Option<ImageData> {
        let data = self.data?;
        unixnotis_core::NotificationImage::normalize_image_data(ImageData {
            width,
            height,
            rowstride,
            has_alpha,
            bits_per_sample,
            channels,
            data,
        })
    }
}

impl<'de> Deserialize<'de> for BoundedImageBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoundedImageBytesVisitor)
    }
}

struct BoundedImageBytesVisitor;

impl<'de> Visitor<'de> for BoundedImageBytesVisitor {
    type Value = BoundedImageBytes;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a bounded notification image byte array")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let retained_limit = unixnotis_core::NotificationImage::retained_byte_limit();
        let mut data = Some(Vec::new());

        while let Some(byte) = sequence.next_element::<u8>()? {
            let Some(retained) = data.as_mut() else {
                continue;
            };
            if retained.len() == retained_limit {
                // Release a partial buffer as soon as the optional image crosses the limit
                data = None;
                continue;
            }
            retained.push(byte);
        }

        Ok(BoundedImageBytes { data })
    }
}
