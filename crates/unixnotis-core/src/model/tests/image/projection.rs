use super::super::{ImageData, NotificationImage};

#[test]
fn listing_projection_removes_raw_image_bytes_but_keeps_identifiers() {
    let image = NotificationImage {
        has_image_data: true,
        image_data: ImageData {
            width: 1,
            height: 1,
            rowstride: 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: vec![9, 8, 7, 6],
        },
        image_path: "/tmp/icon.png".to_string(),
        icon_name: "icon-name".to_string(),
    };

    let listing = image.for_listing();

    assert!(!listing.has_image_data);
    assert!(listing.image_data.data.is_empty());
    assert_eq!(listing.image_path, "/tmp/icon.png");
    assert_eq!(listing.icon_name, "icon-name");
}

#[test]
fn history_projection_drops_raw_data_only_when_alternate_identifier_exists() {
    let with_icon = NotificationImage {
        has_image_data: true,
        image_data: ImageData {
            width: 1,
            height: 1,
            rowstride: 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: vec![1, 2, 3, 4],
        },
        image_path: String::new(),
        icon_name: "app-icon".to_string(),
    };
    let without_icon = NotificationImage {
        icon_name: String::new(),
        ..with_icon.clone()
    };

    assert!(!with_icon.for_history().has_image_data);
    assert!(without_icon.for_history().has_image_data);
}
