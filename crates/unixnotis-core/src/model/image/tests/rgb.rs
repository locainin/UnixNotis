use super::super::rgb::expand_rgb_row_scalar;
#[cfg(target_arch = "x86_64")]
use super::super::rgb::expand_rgb_row_ssse3;
use super::super::{ImageData, NotificationImage, MAX_IMAGE_BYTES};

#[test]
fn expand_rgb_to_rgba_preserves_rows_with_padding() {
    let image = ImageData {
        width: 2,
        height: 2,
        rowstride: 8,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![1, 2, 3, 4, 5, 6, 99, 99, 7, 8, 9, 10, 11, 12, 88, 88],
    };

    let expanded = NotificationImage::expand_rgb_to_rgba(&image).expect("rgb should expand");

    assert_eq!(
        expanded.data,
        vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
    );
    assert_eq!(expanded.rowstride, 8);
}

#[test]
fn expand_rgb_to_rgba_accepts_zero_stride_and_exact_output_limit() {
    let image = ImageData {
        width: 256,
        height: 256,
        rowstride: 0,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![7; 256 * 256 * 3],
    };

    let expanded = NotificationImage::expand_rgb_to_rgba(&image).expect("max rgb should expand");

    assert_eq!(expanded.data.len(), MAX_IMAGE_BYTES);
    assert_eq!(expanded.rowstride, 1024);
}

#[test]
fn expand_rgb_to_rgba_zero_stride_uses_inferred_distinct_rows() {
    let image = ImageData {
        width: 1,
        height: 2,
        rowstride: 0,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![1, 2, 3, 4, 5, 6],
    };

    let expanded = NotificationImage::expand_rgb_to_rgba(&image).expect("rgb should expand");

    assert_eq!(expanded.data, vec![1, 2, 3, 255, 4, 5, 6, 255]);
    assert_eq!(expanded.rowstride, 4);
}

#[test]
fn expand_rgb_to_rgba_rejects_short_padded_rows() {
    let image = ImageData {
        width: 2,
        height: 2,
        rowstride: 8,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![1, 2, 3, 4, 5, 6, 99, 99, 7, 8, 9],
    };

    assert!(NotificationImage::expand_rgb_to_rgba(&image).is_none());
}

#[test]
fn scalar_rgb_expansion_writes_expected_alpha_bytes() {
    let mut out = vec![0; 8];
    expand_rgb_row_scalar(&[1, 2, 3, 4, 5, 6], &mut out);

    assert_eq!(out, vec![1, 2, 3, 255, 4, 5, 6, 255]);
}

#[cfg(target_arch = "x86_64")]
#[test]
fn ssse3_rgb_expansion_matches_scalar_when_supported() {
    if !std::is_x86_feature_detected!("ssse3") {
        return;
    }

    let src = [1_u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    let mut scalar = vec![0; src.len() / 3 * 4];
    let mut simd = vec![0; scalar.len()];

    expand_rgb_row_scalar(&src, &mut scalar);
    // SAFETY: The test is guarded by the same runtime feature probe as production
    unsafe { expand_rgb_row_ssse3(&src, &mut simd) };

    assert_eq!(simd, scalar);
}
