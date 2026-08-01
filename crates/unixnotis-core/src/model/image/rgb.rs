//! RGB-to-RGBA image expansion
//!
//! Uses the scalar path exclusively to keep the module free of unsafe SIMD intrinsics.
//! Modern x86_64 compilers auto-vectorize the hot pixel loop, and notification images
//! are small enough that any performance difference is negligible.

use super::{ImageData, NotificationImage, MAX_IMAGE_BYTES};

impl NotificationImage {
    pub(super) fn expand_rgb_to_rgba(image: &ImageData) -> Option<ImageData> {
        // Expand RGB to RGBA while preserving row semantics and size limits
        let width = image.width.max(1) as usize;
        let height = image.height.max(1) as usize;
        let rowstride = if image.rowstride > 0 {
            image.rowstride as usize
        } else {
            width.checked_mul(3)?
        };
        let pixel_count = width.checked_mul(height)?;
        let output_len = pixel_count.checked_mul(4)?;
        if output_len > MAX_IMAGE_BYTES {
            return None;
        }
        let mut rgba = vec![0u8; output_len];

        for y in 0..height {
            let row_start = y.saturating_mul(rowstride);
            let row_bytes = width.checked_mul(3)?;
            let row_end = row_start.checked_add(row_bytes)?;
            if row_end > image.data.len() {
                return None;
            }
            let row = &image.data[row_start..row_end];
            let dst_start = (y * width) * 4;
            let dst_end = dst_start + width * 4;
            let dst_row = &mut rgba[dst_start..dst_end];
            expand_rgb_row_scalar(row, dst_row);
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

pub(in crate::model) fn expand_rgb_row_scalar(src: &[u8], dst: &mut [u8]) {
    for (x, chunk) in src.chunks_exact(3).enumerate() {
        let dst_index = x * 4;
        // Pack RGBA bytes to reduce bounds checks and stores
        let packed = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], 255]);
        dst[dst_index..dst_index + 4].copy_from_slice(&packed.to_le_bytes());
    }
}
