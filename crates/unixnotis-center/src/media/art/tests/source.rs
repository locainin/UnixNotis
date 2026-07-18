use std::path::PathBuf;

use url::Url;

use super::super::{normalize_art_source, MediaArtKey, MediaArtSource};

#[test]
fn media_art_source_stable_key_keeps_source_kind_visible() {
    let local = MediaArtSource::LocalFile(PathBuf::from("/tmp/art.png"));
    let remote = MediaArtSource::RemoteHttps(
        Url::parse("https://example.com/art.png").expect("test artwork URL should parse"),
    );

    // File and remote art can share a URL-looking body, so the prefix is part of the key
    assert_eq!(
        local.stable_key(),
        MediaArtKey::Local(PathBuf::from("/tmp/art.png"))
    );
    assert_eq!(
        remote.stable_key(),
        MediaArtKey::Remote(
            Url::parse("https://example.com/art.png").expect("test artwork URL should parse")
        )
    );
}

#[cfg(unix)]
#[test]
fn local_media_art_keys_keep_distinct_non_utf8_paths() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let first = MediaArtSource::LocalFile(PathBuf::from(OsString::from_vec(vec![0x80])));
    let second = MediaArtSource::LocalFile(PathBuf::from(OsString::from_vec(vec![0x81])));

    assert_ne!(first.stable_key(), second.stable_key());
}

#[test]
fn artwork_source_normalization_keeps_local_and_allowed_https_inputs() {
    let local = normalize_art_source("file:///tmp/track%20art.png", false);
    assert!(matches!(local, Some(MediaArtSource::LocalFile(_))));

    let localhost = normalize_art_source("file://localhost/tmp/track%20art.png", false);
    assert!(matches!(localhost, Some(MediaArtSource::LocalFile(_))));

    let remote = normalize_art_source("https://example.com/art.png", true);
    assert!(matches!(remote, Some(MediaArtSource::RemoteHttps(_))));
}

#[test]
fn artwork_source_normalization_rejects_disallowed_remote_targets() {
    for value in [
        "http://example.com/art.png",
        "file://player.example/tmp/art.png",
        "https://127.0.0.1/art.png",
        "https://localhost/art.png",
        "https://player.localhost/art.png",
        "https://user@example.com/art.png",
        "https://example.com:8443/art.png",
        "https://example.com/art.png#section",
    ] {
        assert!(normalize_art_source(value, true).is_none(), "{value}");
    }
}
