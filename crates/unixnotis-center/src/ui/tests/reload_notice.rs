use super::*;

fn notice(kind: ReloadNoticeKind, identity: &str) -> ReloadNotice {
    ReloadNotice {
        fingerprint: ReloadNoticeFingerprint {
            kind,
            identity: identity.to_string(),
        },
        message: identity.to_string(),
        error: kind == ReloadNoticeKind::Config,
    }
}

#[test]
fn config_rejection_outranks_a_later_css_failure() {
    let mut state = ReloadNoticeState::default();
    state.set(notice(ReloadNoticeKind::Config, "config-a"));
    state.set(notice(ReloadNoticeKind::Css, "css-a"));

    assert_eq!(
        state.visible().map(|notice| notice.fingerprint.kind),
        Some(ReloadNoticeKind::Config)
    );
}

#[test]
fn dismissed_css_does_not_reopen_after_unrelated_config_recovery() {
    let mut state = ReloadNoticeState::default();
    state.set(notice(ReloadNoticeKind::Css, "css-a"));
    state.dismiss_visible();
    state.clear(ReloadNoticeKind::Config);

    assert!(state.visible().is_none());
}

#[test]
fn changed_css_failure_reopens_after_an_older_failure_was_dismissed() {
    let mut state = ReloadNoticeState::default();
    state.set(notice(ReloadNoticeKind::Css, "css-a"));
    state.dismiss_visible();
    state.set(notice(ReloadNoticeKind::Css, "css-b"));

    assert_eq!(
        state
            .visible()
            .map(|notice| notice.fingerprint.identity.as_str()),
        Some("css-b")
    );
}

#[test]
fn successful_recovery_clears_only_the_matching_notice_kind() {
    let mut state = ReloadNoticeState::default();
    state.set(notice(ReloadNoticeKind::Config, "config-a"));
    state.set(notice(ReloadNoticeKind::Css, "css-a"));
    state.clear(ReloadNoticeKind::Config);

    assert_eq!(
        state.visible().map(|notice| notice.fingerprint.kind),
        Some(ReloadNoticeKind::Css)
    );
    state.clear(ReloadNoticeKind::Css);
    assert!(state.visible().is_none());
}
