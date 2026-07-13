use super::{image_key_matches, set_image_key, IconKey};

fn key(name: &str) -> IconKey {
    IconKey::Name {
        name: name.to_string(),
        size: 24,
        scale: 1,
    }
}

#[gtk::test]
fn image_qdata_key_matches_only_the_stored_icon_request() {
    let image = gtk::Image::new();
    let stored = key("network-wireless");
    let different = key("audio-volume-high");

    assert!(!image_key_matches(&image, &stored));
    set_image_key(&image, stored.clone());

    assert!(image_key_matches(&image, &stored));
    assert!(!image_key_matches(&image, &different));
}
