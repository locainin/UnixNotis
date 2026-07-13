//! RGB-to-RGBA image expansion

#[cfg(target_arch = "x86_64")]
use std::sync::OnceLock;

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

        #[cfg(target_arch = "x86_64")]
        let use_simd = {
            // Cache CPUID once so repeated image hints do not re-run feature detection
            static HAS_SSSE3: OnceLock<bool> = OnceLock::new();
            *HAS_SSSE3.get_or_init(|| std::is_x86_feature_detected!("ssse3"))
        };
        #[cfg(not(target_arch = "x86_64"))]
        let use_simd = false;

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
            if use_simd {
                #[cfg(target_arch = "x86_64")]
                // SAFETY: Guarded by SSSE3 detection; row slices are bounded to full pixels
                unsafe {
                    expand_rgb_row_ssse3(row, dst_row);
                }
                #[cfg(not(target_arch = "x86_64"))]
                expand_rgb_row_scalar(row, dst_row);
            } else {
                expand_rgb_row_scalar(row, dst_row);
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

pub(super) fn expand_rgb_row_scalar(src: &[u8], dst: &mut [u8]) {
    for (x, chunk) in src.chunks_exact(3).enumerate() {
        let dst_index = x * 4;
        // Pack RGBA bytes to reduce bounds checks and stores
        let packed = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], 255]);
        dst[dst_index..dst_index + 4].copy_from_slice(&packed.to_le_bytes());
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
#[expect(
    clippy::cast_ptr_alignment,
    reason = "the SSSE3 loadu and storeu intrinsics explicitly support unaligned byte buffers"
)]
pub(super) unsafe fn expand_rgb_row_ssse3(src: &[u8], dst: &mut [u8]) {
    // SSSE3 shuffles 12-byte RGB quads into 16-byte RGBA blocks with a fixed alpha mask
    use std::arch::x86_64::{
        __m128i, _mm_loadu_si128, _mm_or_si128, _mm_setr_epi8, _mm_shuffle_epi8, _mm_storeu_si128,
    };

    let mut s = 0usize;
    let mut d = 0usize;

    let mask: __m128i = _mm_setr_epi8(0, 1, 2, -128, 3, 4, 5, -128, 6, 7, 8, -128, 9, 10, 11, -128);
    let alpha: __m128i = _mm_setr_epi8(0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1);

    // Process 4 pixels at a time (12 bytes -> 16 bytes). Read requires 16 bytes
    while s + 16 <= src.len() {
        // SAFETY: The loop guard proves the 16-byte unaligned read remains inside src
        let src_ptr = unsafe { src.as_ptr().add(s) };
        // SAFETY: SSSE3 permits this pointer to be unaligned
        let chunk = unsafe { _mm_loadu_si128(src_ptr.cast::<__m128i>()) };
        let shuffled = _mm_shuffle_epi8(chunk, mask);
        let with_alpha = _mm_or_si128(shuffled, alpha);
        // SAFETY: Four source pixels always map to the next 16-byte destination block
        let dst_ptr = unsafe { dst.as_mut_ptr().add(d) };
        // SAFETY: The caller allocates four RGBA bytes for every source RGB pixel
        unsafe { _mm_storeu_si128(dst_ptr.cast::<__m128i>(), with_alpha) };
        s += 12;
        d += 16;
    }

    // Tail handles the remaining one to three pixels
    let remaining_pixels = (src.len().saturating_sub(s)) / 3;
    for index in 0..remaining_pixels {
        let s = s + index * 3;
        let d = d + index * 4;
        dst[d] = src[s];
        dst[d + 1] = src[s + 1];
        dst[d + 2] = src[s + 2];
        dst[d + 3] = 255;
    }
}
