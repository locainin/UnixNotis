use std::path::PathBuf;

use url::Url;

use super::MediaArtSource;

#[test]
fn media_art_source_stable_key_keeps_source_kind_visible() {
    let local = MediaArtSource::LocalFile(PathBuf::from("/tmp/art.png"));
    let remote = MediaArtSource::RemoteHttps(Url::parse("https://example.com/art.png").unwrap());

    // File and remote art can share a URL-looking body, so the prefix is part of the key
    assert_eq!(local.stable_key(), "file:/tmp/art.png");
    assert_eq!(remote.stable_key(), "https:https://example.com/art.png");
}
