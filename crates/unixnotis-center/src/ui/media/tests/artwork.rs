use super::{art_dimensions_allowed, MediaArtCompletion, MediaArtState};

#[test]
fn art_dimensions_allowed_rejects_non_images() {
    assert!(super::art_dimensions_from_bytes(b"not-an-image").is_none());
}

#[test]
fn art_dimensions_allowed_rejects_oversized_images() {
    assert!(!art_dimensions_allowed(4096, 1024));
}

#[test]
fn same_displayed_key_cancels_pending_work() {
    let mut state = MediaArtState {
        displayed_key: Some("cover-a".to_string()),
        pending_key: Some("cover-b".to_string()),
        pending_gen: 7,
    };

    assert!(state.keep_displayed_if_current(&Some("cover-a".to_string())));
    assert_eq!(state.displayed_key.as_deref(), Some("cover-a"));
    assert_eq!(state.pending_key, None);
    assert_eq!(state.pending_gen, 8);
}

#[test]
fn changed_key_failure_does_not_poison_same_key_retry() {
    let mut state = MediaArtState::default();
    let key = Some("cover-b".to_string());

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
    let old_key = Some("cover-a".to_string());
    let new_key = Some("cover-b".to_string());

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
        displayed_key: Some("cover-a".to_string()),
        pending_key: Some("cover-b".to_string()),
        pending_gen: 11,
    };

    state.clear_displayed_now();

    assert_eq!(state.displayed_key, None);
    assert_eq!(state.pending_key, None);
    assert_eq!(state.pending_gen, 12);
}
