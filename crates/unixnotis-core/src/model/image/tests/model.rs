use super::super::{
    ImageData, NotificationImage, NotificationVisualRole, MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION,
};

#[test]
fn image_models_default_to_empty_bounded_payloads() {
    let image = NotificationImage::default();
    assert!(image.badge_icon.is_empty());
    assert_eq!(image.sender_visual_role, NotificationVisualRole::None);
    assert!(image.sender_visual.data.is_empty());
    assert!(image.content_image.data.is_empty());
    assert_eq!(MAX_IMAGE_BYTES, 256 * 1024);
    assert_eq!(MAX_IMAGE_DIMENSION, 256);
    assert_eq!(NotificationImage::retained_byte_limit(), MAX_IMAGE_BYTES);
    assert_eq!(ImageData::default().width, 0);
}
