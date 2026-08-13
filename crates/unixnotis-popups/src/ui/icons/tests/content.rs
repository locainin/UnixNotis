use super::*;

fn image_data(channels: i32, rowstride: i32, data: Vec<u8>) -> NotificationImage {
    NotificationImage {
        claimed_desktop_id: String::new(),
        content_image: ImageData {
            width: 2,
            height: 1,
            rowstride,
            has_alpha: channels == 4,
            bits_per_sample: 8,
            channels,
            data,
        },
        ..NotificationImage::default()
    }
}

#[test]
fn rgb_content_image_expands_to_opaque_rgba() {
    let image = image_data(3, 8, vec![1, 2, 3, 4, 5, 6, 90, 91]);

    let (bytes, stride) = expand_rgb_to_rgba(&image.content_image).expect("valid RGB data");

    assert_eq!(stride, 8);
    assert_eq!(bytes, vec![1, 2, 3, 255, 4, 5, 6, 255]);
}

#[gtk::test]
fn valid_rgba_content_image_creates_a_texture() {
    let image = image_data(4, 8, vec![1, 2, 3, 4, 5, 6, 7, 8]);

    assert!(image_data_texture(&image).is_some());
}

#[gtk::test]
fn undersized_content_buffer_is_rejected() {
    let image = image_data(4, 8, vec![1, 2, 3, 4]);

    assert!(image_data_texture(&image).is_none());
}
