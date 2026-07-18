use super::{hash_image_data, icon_key_for_path, image_key_matches, set_image_key, IconKey};

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

#[test]
fn image_data_hash_changes_when_only_the_middle_bytes_change() {
    let mut first = vec![0x11; 16_384];
    let mut second = first.clone();
    first[8_192] = 0x22;
    second[8_192] = 0x33;

    // Matching boundaries must not hide a changed pixel payload from the cache key
    assert_eq!(&first[..64], &second[..64]);
    assert_eq!(&first[first.len() - 64..], &second[second.len() - 64..]);
    assert_ne!(hash_image_data(&first), hash_image_data(&second));
}

#[cfg(unix)]
#[test]
fn file_icon_keys_keep_distinct_non_utf8_paths() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    let first = PathBuf::from(OsString::from_vec(vec![b'i', b'c', b'o', b'n', 0x80]));
    let second = PathBuf::from(OsString::from_vec(vec![b'i', b'c', b'o', b'n', 0x81]));

    assert_ne!(
        icon_key_for_path(&first, 24, 1),
        icon_key_for_path(&second, 24, 1)
    );
}
