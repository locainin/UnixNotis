use super::super::{owned_to_string, strip_desktop_suffix, NotificationImage};
use super::{image_data_value, string_value};
use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Structure, Value};

#[test]
fn from_hints_prefers_valid_image_data_over_image_path_and_icon() {
    let mut hints = HashMap::new();
    hints.insert(
        "image-data".to_string(),
        image_data_value(1, 1, 4, true, 8, 4, vec![1, 2, 3, 4]),
    );
    hints.insert("image-path".to_string(), string_value("/tmp/icon.png"));

    let image = NotificationImage::from_hints("App", "fallback-icon", &hints);

    assert!(image.has_image_data);
    assert_eq!(image.image_data.data, vec![1, 2, 3, 4]);
    assert_eq!(image.image_path, "/tmp/icon.png");
    assert_eq!(image.icon_name, "fallback-icon");
}

#[test]
fn from_hints_falls_back_from_invalid_image_data_to_app_icon_path() {
    let mut hints = HashMap::new();
    hints.insert(
        "image-data".to_string(),
        image_data_value(0, 1, 4, true, 8, 4, vec![1, 2, 3, 4]),
    );

    let image = NotificationImage::from_hints("App", "/tmp/app-icon.png", &hints);

    assert!(!image.has_image_data);
    assert_eq!(image.image_path, "/tmp/app-icon.png");
    assert!(image.icon_name.is_empty());
}

#[test]
fn from_hints_uses_desktop_entry_before_app_name_for_icon_name() {
    let mut hints = HashMap::new();
    hints.insert(
        "desktop-entry".to_string(),
        string_value("org.example.App.desktop"),
    );

    let image = NotificationImage::from_hints("Fallback App", "", &hints);

    assert_eq!(image.icon_name, "org.example.App");
}

#[test]
fn from_hints_uses_app_name_when_no_icon_hints_exist() {
    let hints = HashMap::new();

    let image = NotificationImage::from_hints("Fallback App", "", &hints);

    assert_eq!(image.icon_name, "Fallback App");
    assert!(image.image_path.is_empty());
    assert!(!image.has_image_data);
}

#[test]
fn parse_image_data_accepts_legacy_hint_aliases_and_rejects_wrong_field_count() {
    let parsed = NotificationImage::parse_image_data(&image_data_value(
        1,
        1,
        4,
        true,
        8,
        4,
        vec![1, 2, 3, 4],
    ))
    .expect("valid image-data should parse");
    assert_eq!(parsed.width, 1);
    assert_eq!(parsed.channels, 4);

    let wrong = Structure::from((1_i32, 1_i32));
    let wrong: OwnedValue = Value::from(wrong)
        .try_into()
        .expect("wrong structure should convert");
    assert!(NotificationImage::parse_image_data(&wrong).is_none());
}

#[test]
fn array_to_bytes_rejects_empty_large_and_non_byte_arrays() {
    let empty = Value::from(Vec::<u8>::new());
    assert!(NotificationImage::array_to_bytes(&empty).is_none());

    let too_large = Value::from(vec![0_u8; super::super::MAX_IMAGE_BYTES + 1]);
    assert!(NotificationImage::array_to_bytes(&too_large).is_none());

    let exact_limit = Value::from(vec![0_u8; super::super::MAX_IMAGE_BYTES]);
    assert_eq!(
        NotificationImage::array_to_bytes(&exact_limit)
            .expect("exact limit should be accepted")
            .len(),
        super::super::MAX_IMAGE_BYTES
    );

    let wrong_type = Value::from(vec![1_u32]);
    assert!(NotificationImage::array_to_bytes(&wrong_type).is_none());

    let bytes = Value::from(vec![1_u8, 2, 3]);
    assert_eq!(
        NotificationImage::array_to_bytes(&bytes),
        Some(vec![1, 2, 3])
    );
}

#[test]
fn owned_string_and_desktop_suffix_helpers_match_hint_expectations() {
    assert_eq!(
        owned_to_string(&string_value("org.example.App.desktop")).as_deref(),
        Some("org.example.App.desktop")
    );
    assert_eq!(
        strip_desktop_suffix("org.example.App.desktop"),
        "org.example.App"
    );
    assert_eq!(strip_desktop_suffix("org.example.App"), "org.example.App");
}
