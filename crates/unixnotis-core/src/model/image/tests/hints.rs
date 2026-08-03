use super::super::{ImageData, NotificationImage, NotificationVisualRole, MAX_IMAGE_BYTES};
use super::{image_data_value, string_value};
use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Structure, Value};

#[test]
fn embedded_image_data_is_retained_as_content() {
    let mut hints = HashMap::new();
    hints.insert(
        "image-data".to_string(),
        image_data_value(1, 1, 4, true, 8, 4, vec![1, 2, 3, 4]),
    );
    hints.insert("image-path".to_string(), string_value("/tmp/icon.png"));

    let image = NotificationImage::from_hints("App", "/tmp/app.png", &hints);
    assert_eq!(image.sender_visual_role, NotificationVisualRole::None);
    assert_eq!(image.content_image.data, vec![1, 2, 3, 4]);
    assert!(image.badge_icon.is_empty());
}

#[test]
fn app_icon_and_image_path_never_become_retained_host_paths() {
    let mut hints = HashMap::new();
    hints.insert("image-path".to_string(), string_value("/tmp/icon.png"));
    let image = NotificationImage::from_hints("App", "/tmp/app.png", &hints);
    assert!(image.sender_visual.data.is_empty());
    assert!(image.content_image.data.is_empty());
}

#[test]
fn app_icon_theme_names_are_retained_only_as_bounded_lookup_hints() {
    let image = NotificationImage::from_hints("App", "example-player", &HashMap::new());
    assert_eq!(image.claimed_theme_icon, "example-player");

    for value in [
        "/tmp/icon.png",
        "file:///tmp/icon.png",
        "../icon",
        "icon name",
        "icon:remote",
    ] {
        let image = NotificationImage::from_hints("App", value, &HashMap::new());
        assert!(image.claimed_theme_icon.is_empty(), "unsafe hint: {value}");
    }
}

#[test]
fn parse_image_data_rejects_wrong_structure() {
    let wrong = Structure::from((1_i32, 1_i32));
    let wrong: OwnedValue = Value::from(wrong).try_into().expect("structure conversion");
    assert!(NotificationImage::parse_image_data(&wrong).is_none());
}

#[test]
fn parse_image_data_accepts_legacy_aliases() {
    let value = image_data_value(1, 1, 4, true, 8, 4, vec![1, 2, 3, 4]);
    let parsed = NotificationImage::parse_image_data(&value).expect("valid image data");
    assert_eq!(
        parsed,
        ImageData {
            width: 1,
            height: 1,
            rowstride: 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: vec![1, 2, 3, 4],
        }
    );
}

#[test]
fn parse_image_data_enforces_the_exact_raw_byte_boundary() {
    let accepted = image_data_value(1, 1, 4, true, 8, 4, vec![0; MAX_IMAGE_BYTES]);
    assert!(NotificationImage::parse_image_data(&accepted).is_some());

    let rejected = image_data_value(1, 1, 4, true, 8, 4, vec![0; MAX_IMAGE_BYTES + 1]);
    assert!(NotificationImage::parse_image_data(&rejected).is_none());
}

#[test]
fn array_to_bytes_rejects_empty_payloads() {
    let value = Value::from(Vec::<u8>::new());

    assert!(NotificationImage::array_to_bytes(&value).is_none());
}
