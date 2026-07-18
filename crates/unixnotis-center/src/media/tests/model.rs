use std::path::PathBuf;

use url::Url;

use super::super::{MediaArtKey, MediaArtSource};

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
