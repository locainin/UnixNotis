use super::super::{
    ImageData, NotificationImage, MAX_ICON_NAME_BYTES, MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION,
    MAX_IMAGE_PATH_BYTES,
};

#[test]
fn image_models_default_to_empty_bounded_payloads() {
    let data = ImageData::default();
    let image = NotificationImage::default();

    assert_eq!(data.width, 0);
    assert!(data.data.is_empty());
    assert!(!image.has_image_data);
    assert!(image.image_path.is_empty());
    assert!(image.icon_name.is_empty());
    assert_eq!(MAX_IMAGE_BYTES, 256 * 1024);
    assert_eq!(MAX_IMAGE_DIMENSION, 256);
    assert_eq!(MAX_IMAGE_PATH_BYTES, 1024);
    assert_eq!(MAX_ICON_NAME_BYTES, 256);
}
