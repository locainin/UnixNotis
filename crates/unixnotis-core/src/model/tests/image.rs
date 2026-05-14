use super::{ImageData, NotificationImage};

#[test]
fn normalize_image_data_rejects_short_rowstride() {
    // Rowstride shorter than width * 4 must be rejected to avoid invalid layouts.
    let image = ImageData {
        width: 2,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![0u8; 8],
    };
    assert!(NotificationImage::normalize_image_data(image).is_none());
}

#[test]
fn normalize_image_data_rejects_short_buffer() {
    // Buffer smaller than rowstride * height must be rejected.
    let image = ImageData {
        width: 2,
        height: 2,
        rowstride: 8,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![0u8; 8],
    };
    assert!(NotificationImage::normalize_image_data(image).is_none());
}

#[test]
fn normalize_image_data_accepts_valid_rgba() {
    // Rowstride 0 should normalize to width * 4 when data length matches.
    let image = ImageData {
        width: 2,
        height: 1,
        rowstride: 0,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![0u8; 8],
    };
    let normalized = NotificationImage::normalize_image_data(image).expect("valid image data");
    assert_eq!(normalized.rowstride, 8);
}

#[test]
fn normalize_image_data_expands_rgb() {
    // RGB input should expand to RGBA with the expected output size.
    let image = ImageData {
        width: 2,
        height: 1,
        rowstride: 0,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![10, 20, 30, 40, 50, 60],
    };
    let normalized = NotificationImage::normalize_image_data(image).expect("expanded image");
    assert_eq!(normalized.channels, 4);
    assert_eq!(normalized.data.len(), 8);
}
