use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::prelude::WidgetExt;

use super::{
    art_dimensions_allowed, load_art_bytes, MediaArtCompletion, MediaArtState, MAX_LOCAL_ART_BYTES,
    MAX_MEDIA_ART_DECODE_ALLOC_BYTES,
};

use crate::media::{MediaArtKey, MediaArtSource};
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};

#[test]
fn art_dimensions_allowed_rejects_non_images() {
    assert!(super::art_dimensions_from_bytes(b"not-an-image").is_none());
}

#[test]
fn art_dimensions_allowed_rejects_oversized_images() {
    assert!(!art_dimensions_allowed(4096, 1024));
}

#[test]
fn art_decoder_accepts_a_bounded_raster_with_decoder_limits() {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&[1, 2, 3, 255], 1, 1, ExtendedColorType::Rgba8)
        .expect("encode artwork PNG");

    let decoded = super::decode_art_raster(bytes).expect("decode bounded artwork");

    assert_eq!((decoded.width, decoded.height, decoded.stride), (1, 1, 4));
    assert_eq!(decoded.bytes, vec![1, 2, 3, 255]);
}

#[test]
fn artwork_byte_and_decode_budgets_keep_the_documented_limits() {
    assert_eq!(MAX_LOCAL_ART_BYTES, 8 * 1_024 * 1_024);
    assert_eq!(MAX_MEDIA_ART_DECODE_ALLOC_BYTES, 32 * 1_024 * 1_024);
}

#[test]
fn local_art_loader_returns_the_complete_nonempty_file() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "unixnotis-center-art-loader-{}-{stamp}.png",
        std::process::id()
    ));
    let expected = b"complete local art bytes";
    fs::write(&path, expected).expect("write local art fixture");
    let source = MediaArtSource::LocalFile(path.clone());

    let loaded = glib::MainContext::new().block_on(load_art_bytes(&source));

    assert_eq!(loaded.as_deref(), Some(expected.as_slice()));
    fs::remove_file(path).expect("remove local art fixture");
}

#[gtk::test]
fn apply_media_art_without_a_source_clears_visible_and_pending_state() {
    let picture = gtk::Picture::new();
    let state = Rc::new(RefCell::new(MediaArtState {
        displayed_key: Some(art_key("visible")),
        pending_key: Some(art_key("pending")),
        pending_gen: 4,
    }));

    super::apply_media_art(&picture, &state, None);

    let state = state.borrow();
    assert_eq!(state.displayed_key, None);
    assert_eq!(state.pending_key, None);
    assert_eq!(state.pending_gen, 5);
    assert!(picture.has_css_class("empty"));
}

#[cfg(unix)]
#[test]
fn local_art_loader_rejects_last_component_symlinks() {
    use std::os::unix::fs::symlink;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-center-art-symlink-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create artwork test root");
    let target = root.join("target.png");
    let link = root.join("link.png");
    fs::write(&target, b"art bytes").expect("write artwork target");
    symlink(&target, &link).expect("create artwork symlink");
    let source = MediaArtSource::LocalFile(link);

    let loaded = glib::MainContext::new().block_on(load_art_bytes(&source));

    assert_eq!(loaded, None);
    fs::remove_dir_all(root).expect("remove artwork test root");
}

#[test]
fn same_displayed_key_cancels_pending_work() {
    let mut state = MediaArtState {
        displayed_key: Some(art_key("cover-a")),
        pending_key: Some(art_key("cover-b")),
        pending_gen: 7,
    };

    assert!(state.keep_displayed_if_current(&Some(art_key("cover-a"))));
    assert_eq!(state.displayed_key, Some(art_key("cover-a")));
    assert_eq!(state.pending_key, None);
    assert_eq!(state.pending_gen, 8);
}

#[test]
fn changed_key_failure_does_not_poison_same_key_retry() {
    let mut state = MediaArtState::default();
    let key = Some(art_key("cover-b"));

    let request_gen = state.begin_request(key.clone());
    assert_eq!(
        state.finish_request(request_gen, key.clone(), false),
        MediaArtCompletion::Clear
    );
    assert_eq!(state.displayed_key, None);
    assert_eq!(state.pending_key, None);
    assert!(!state.keep_displayed_if_current(&key));
    assert!(!state.pending_key_matches(&key));
}

#[test]
fn stale_completion_cannot_overwrite_newer_request() {
    let mut state = MediaArtState::default();
    let old_key = Some(art_key("cover-a"));
    let new_key = Some(art_key("cover-b"));

    let old_gen = state.begin_request(old_key.clone());
    let new_gen = state.begin_request(new_key.clone());

    assert_eq!(
        state.finish_request(old_gen, old_key, true),
        MediaArtCompletion::Ignore
    );
    assert_eq!(
        state.finish_request(new_gen, new_key.clone(), true),
        MediaArtCompletion::Apply
    );
    assert_eq!(state.displayed_key, new_key);
}

#[test]
fn clear_now_invalidates_inflight_requests() {
    let mut state = MediaArtState {
        displayed_key: Some(art_key("cover-a")),
        pending_key: Some(art_key("cover-b")),
        pending_gen: 11,
    };

    state.clear_displayed_now();

    assert_eq!(state.displayed_key, None);
    assert_eq!(state.pending_key, None);
    assert_eq!(state.pending_gen, 12);
}

fn art_key(name: &str) -> MediaArtKey {
    MediaArtKey::Local(std::path::PathBuf::from(name))
}
