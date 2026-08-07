//! Allocation-bounded byte-array decoding for optional notification images

use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use unixnotis_core::{ImageData, NotificationImage};

use super::super::notify_body::{MAX_NOTIFY_WIRE_IMAGE_BYTES, MAX_NOTIFY_WIRE_IMAGE_DIMENSION};

/// Raw image bytes retained only under the D-Bus wire budget
#[derive(Debug, Default)]
pub(super) struct BoundedImageBytes {
    data: Option<Vec<u8>>,
}

/// Validated wire pixels that have not entered the retained notification model
#[derive(Debug)]
pub(in crate::daemon::notifications::server) struct WireImageData {
    width: u32,
    height: u32,
    rowstride: usize,
    channels: u8,
    data: Vec<u8>,
}

impl WireImageData {
    pub(in crate::daemon::notifications::server) fn from_parts(
        width: i32,
        height: i32,
        rowstride: i32,
        _has_alpha: bool,
        bits_per_sample: i32,
        channels: i32,
        data: Vec<u8>,
    ) -> Option<Self> {
        // Reject metadata before any pixel index is calculated
        if bits_per_sample != 8 {
            return None;
        }
        let width = u32::try_from(width).ok()?;
        let height = u32::try_from(height).ok()?;
        if width == 0
            || height == 0
            || width > MAX_NOTIFY_WIRE_IMAGE_DIMENSION
            || height > MAX_NOTIFY_WIRE_IMAGE_DIMENSION
        {
            return None;
        }
        let channels = u8::try_from(channels).ok()?;
        if !matches!(channels, 3 | 4) {
            return None;
        }
        if data.is_empty() || data.len() > MAX_NOTIFY_WIRE_IMAGE_BYTES {
            return None;
        }

        // The stride must cover every visible pixel in every row
        let width_usize = usize::try_from(width).ok()?;
        let height_usize = usize::try_from(height).ok()?;
        let channels_usize = usize::from(channels);
        let minimum_rowstride = width_usize.checked_mul(channels_usize)?;
        let rowstride = usize::try_from(rowstride).ok()?;
        if rowstride < minimum_rowstride {
            return None;
        }
        let required_bytes = rowstride.checked_mul(height_usize)?;
        if data.len() < required_bytes {
            return None;
        }

        // Extra row padding stays transient and is discarded during output sampling
        Some(Self {
            width,
            height,
            rowstride,
            channels,
            data,
        })
    }

    pub(in crate::daemon::notifications::server) fn into_storage_image(
        self,
        requested_dimension: u32,
    ) -> Option<ImageData> {
        // Clamp the requested output to the persistent model's dimension policy
        let model_dimension = u32::try_from(NotificationImage::retained_dimension_limit()).ok()?;
        let target_dimension = requested_dimension.min(model_dimension);
        if target_dimension == 0 {
            return None;
        }

        let Self {
            width,
            height,
            rowstride,
            channels,
            data,
        } = self;
        let (target_width, target_height) = target_dimensions(width, height, target_dimension)?;
        let target_pixels = usize::try_from(target_width)
            .ok()?
            .checked_mul(usize::try_from(target_height).ok()?)?;
        let output_len = target_pixels.checked_mul(4)?;
        let mut rgba = vec![0_u8; output_len];
        let channels = usize::from(channels);
        let source_width = usize::try_from(width).ok()?;
        let source_height = usize::try_from(height).ok()?;
        let target_width_usize = usize::try_from(target_width).ok()?;
        let target_height_usize = usize::try_from(target_height).ok()?;

        // Sample source pixels directly so a large wire raster never becomes a second full copy
        for target_y in 0..target_height_usize {
            let source_y = target_y
                .checked_mul(source_height)?
                .checked_div(target_height_usize)?;
            for target_x in 0..target_width_usize {
                let source_x = target_x
                    .checked_mul(source_width)?
                    .checked_div(target_width_usize)?;
                let source_index = source_y
                    .checked_mul(rowstride)?
                    .checked_add(source_x.checked_mul(channels)?)?;
                let source_end = source_index.checked_add(channels)?;
                let source_pixel = data.get(source_index..source_end)?;
                let target_index = target_y
                    .checked_mul(target_width_usize)?
                    .checked_add(target_x)?
                    .checked_mul(4)?;
                let target_pixel = rgba.get_mut(target_index..target_index + 4)?;
                target_pixel[..3].copy_from_slice(&source_pixel[..3]);
                target_pixel[3] = if channels == 4 { source_pixel[3] } else { 255 };
            }
        }

        // The retained validator remains the final model boundary after downsampling
        let width = i32::try_from(target_width).ok()?;
        let height = i32::try_from(target_height).ok()?;
        let rowstride = width.checked_mul(4)?;
        NotificationImage::normalize_image_data(ImageData {
            width,
            height,
            rowstride,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: rgba,
        })
    }
}

impl BoundedImageBytes {
    pub(super) fn into_wire_image(
        self,
        width: i32,
        height: i32,
        rowstride: i32,
        has_alpha: bool,
        bits_per_sample: i32,
        channels: i32,
    ) -> Option<WireImageData> {
        let data = self.data?;
        WireImageData::from_parts(
            width,
            height,
            rowstride,
            has_alpha,
            bits_per_sample,
            channels,
            data,
        )
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
        // The wire limit is separate from the smaller persistent image budget
        let wire_image_limit = MAX_NOTIFY_WIRE_IMAGE_BYTES;
        let mut data = Some(Vec::new());

        while let Some(byte) = sequence.next_element::<u8>()? {
            let Some(retained) = data.as_mut() else {
                continue;
            };
            if retained.len() == wire_image_limit {
                // Release a partial buffer as soon as the wire allowance is crossed
                data = None;
                continue;
            }
            retained.push(byte);
        }

        Ok(BoundedImageBytes { data })
    }
}

fn target_dimensions(width: u32, height: u32, target_dimension: u32) -> Option<(u32, u32)> {
    // Preserve source proportions while keeping both output axes within the target
    if width >= height {
        Some((
            target_dimension.min(width),
            scaled_dimension(height, width, target_dimension.min(width)),
        ))
    } else {
        Some((
            scaled_dimension(width, height, target_dimension.min(height)),
            target_dimension.min(height),
        ))
    }
}

fn scaled_dimension(value: u32, source_dimension: u32, target_dimension: u32) -> u32 {
    if source_dimension <= target_dimension {
        return value;
    }
    // Checked arithmetic keeps future limit changes from turning geometry into a wrap
    u64::from(value)
        .checked_mul(u64::from(target_dimension))
        .and_then(|scaled| scaled.checked_div(u64::from(source_dimension)))
        .and_then(|scaled| u32::try_from(scaled).ok())
        .unwrap_or(1)
        .max(1)
}

#[cfg(test)]
#[path = "tests/image_bytes.rs"]
mod tests;
