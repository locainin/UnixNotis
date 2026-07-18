use unixnotis_core::NotificationImage;

use super::super::{expand_rgb_to_rgba, image_data_texture};
use super::support::image_data;

#[test]
fn expand_rgb_to_rgba_appends_alpha() {
    let data = image_data(2, 1, 0, 3, vec![10, 20, 30, 40, 50, 60]);

    let (expanded, stride) = expand_rgb_to_rgba(&data).expect("rgb expansion");

    assert_eq!(stride, 8);
    assert_eq!(expanded, vec![10, 20, 30, 255, 40, 50, 60, 255]);
}

#[test]
fn expand_rgb_to_rgba_honors_row_padding() {
    let data = image_data(
        2,
        2,
        8,
        3,
        vec![1, 2, 3, 4, 5, 6, 0, 0, 7, 8, 9, 10, 11, 12, 0, 0],
    );

    let (expanded, stride) = expand_rgb_to_rgba(&data).expect("rgb expansion");

    assert_eq!(stride, 8);
    assert_eq!(
        expanded,
        vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
    );
}

#[test]
fn expand_rgb_to_rgba_rejects_empty_dimensions_short_rows_and_short_buffers() {
    assert!(expand_rgb_to_rgba(&image_data(0, 1, 0, 3, vec![1, 2, 3])).is_none());
    assert!(expand_rgb_to_rgba(&image_data(1, 0, 0, 3, vec![1, 2, 3])).is_none());
    assert!(expand_rgb_to_rgba(&image_data(2, 1, 5, 3, vec![1, 2, 3, 4, 5])).is_none());
    assert!(expand_rgb_to_rgba(&image_data(2, 2, 0, 3, vec![1, 2, 3, 4, 5, 6])).is_none());
}

#[gtk::test]
fn image_data_texture_accepts_valid_rgba_and_rgb_payloads() {
    let rgba = NotificationImage {
        has_image_data: true,
        image_data: image_data(1, 1, 0, 4, vec![1, 2, 3, 4]),
        ..NotificationImage::default()
    };
    let rgb = NotificationImage {
        has_image_data: true,
        image_data: image_data(1, 1, 0, 3, vec![1, 2, 3]),
        ..NotificationImage::default()
    };

    assert!(image_data_texture(&rgba).is_some());
    assert!(image_data_texture(&rgb).is_some());
}

#[gtk::test]
fn image_data_texture_rejects_missing_flag_bad_bits_dimensions_and_channels() {
    let mut image = NotificationImage {
        has_image_data: false,
        image_data: image_data(1, 1, 0, 4, vec![1, 2, 3, 4]),
        ..NotificationImage::default()
    };
    assert!(image_data_texture(&image).is_none());

    image.has_image_data = true;
    image.image_data.bits_per_sample = 16;
    assert!(image_data_texture(&image).is_none());

    image.image_data.bits_per_sample = 8;
    image.image_data.width = 0;
    assert!(image_data_texture(&image).is_none());

    image.image_data.width = 1;
    image.image_data.height = -1;
    assert!(image_data_texture(&image).is_none());

    image.image_data.height = 1;
    image.image_data.channels = 2;
    assert!(image_data_texture(&image).is_none());
}

#[gtk::test]
fn image_data_texture_rejects_bad_stride_and_short_buffers() {
    let mut image = NotificationImage {
        has_image_data: true,
        image_data: image_data(2, 1, 7, 4, vec![0; 8]),
        ..NotificationImage::default()
    };
    assert!(image_data_texture(&image).is_none());

    image.image_data.rowstride = -1;
    assert!(image_data_texture(&image).is_none());

    image.image_data.rowstride = 0;
    image.image_data.data = vec![0; 7];
    assert!(image_data_texture(&image).is_none());
}
