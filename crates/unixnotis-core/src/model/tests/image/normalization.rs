use super::super::{ImageData, NotificationImage, MAX_IMAGE_BYTES};

#[test]
fn normalize_image_data_rejects_short_rowstride() {
    // Rowstride shorter than width * 4 must be rejected to avoid invalid layouts
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
    // Buffer smaller than rowstride * height must be rejected
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
    // Rowstride 0 should normalize to width * 4 when data length matches
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
    // RGB input should expand to RGBA with the expected output size
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
    assert_eq!(normalized.data, vec![10, 20, 30, 255, 40, 50, 60, 255]);
    assert_eq!(normalized.rowstride, 8);
    assert!(normalized.has_alpha);
}

#[test]
fn image_data_usability_rejects_invalid_geometry_and_format() {
    let base = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![1, 2, 3, 4],
    };

    assert!(NotificationImage::is_image_data_usable(&base));
    assert!(!NotificationImage::is_image_data_usable(&ImageData {
        width: 0,
        ..base.clone()
    }));
    assert!(!NotificationImage::is_image_data_usable(&ImageData {
        height: 257,
        data: vec![0; 257 * 4],
        ..base
    }));
    assert!(!NotificationImage::is_image_data_usable(&ImageData {
        bits_per_sample: 16,
        ..base.clone()
    }));
    assert!(!NotificationImage::is_image_data_usable(&ImageData {
        channels: 3,
        ..base
    }));
}

#[test]
fn image_data_usability_accepts_max_dimensions_and_rejects_above_max() {
    let rowstride = 256 * 4;
    let max_image = ImageData {
        width: 256,
        height: 256,
        rowstride,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![0; 256 * rowstride as usize],
    };

    assert!(NotificationImage::is_image_data_usable(&max_image));
    assert!(!NotificationImage::is_image_data_usable(&ImageData {
        width: 257,
        ..max_image.clone()
    }));
    assert!(!NotificationImage::is_image_data_usable(&ImageData {
        height: 257,
        ..max_image
    }));
}

#[test]
fn normalized_rowstride_rejects_invalid_lengths_and_infers_zero_stride() {
    assert_eq!(
        NotificationImage::normalized_rowstride(2, 2, 0, 8, 4, 16),
        Some(8)
    );
    assert!(NotificationImage::normalized_rowstride(2, 2, -1, 8, 4, 16).is_none());
    assert!(NotificationImage::normalized_rowstride(2, 2, 7, 8, 4, 16).is_none());
    assert!(NotificationImage::normalized_rowstride(2, 2, 8, 8, 4, 15).is_none());
    assert!(NotificationImage::normalized_rowstride(0, 2, 8, 8, 4, 16).is_none());
    assert_eq!(
        NotificationImage::normalized_rowstride(256, 256, 1024, 8, 4, MAX_IMAGE_BYTES),
        Some(1024)
    );
    assert!(
        NotificationImage::normalized_rowstride(256, 256, 1024, 8, 4, MAX_IMAGE_BYTES + 1)
            .is_none()
    );
}

#[test]
fn bytes_per_pixel_accepts_whole_bytes_only() {
    assert_eq!(NotificationImage::bytes_per_pixel(8, 4), Some(4));
    assert_eq!(NotificationImage::bytes_per_pixel(8, 3), Some(3));
    assert_eq!(NotificationImage::bytes_per_pixel(1, 3), None);
    assert_eq!(NotificationImage::bytes_per_pixel(0, 4), None);
}

#[test]
fn max_image_bytes_constant_stays_at_256_kib() {
    // The cap bounds untrusted D-Bus image payload memory
    assert_eq!(MAX_IMAGE_BYTES, 262_144);
}
