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

#[test]
fn dismissed_config_failure_reopens_after_successful_recovery() {
    let mut state = ReloadNoticeState::default();
    state.set(notice(ReloadNoticeKind::Config, "config-a"));
    state.dismiss_visible();
    state.clear(ReloadNoticeKind::Config);

    state.set(notice(ReloadNoticeKind::Config, "config-a"));

    assert_eq!(
        state
            .visible()
            .map(|notice| notice.fingerprint.identity.as_str()),
        Some("config-a")
    );
}

#[test]
fn dismissed_css_failure_reopens_after_successful_recovery() {
    let mut state = ReloadNoticeState::default();
    state.set(notice(ReloadNoticeKind::Css, "css-a"));
    state.dismiss_visible();
    state.clear(ReloadNoticeKind::Css);

    state.set(notice(ReloadNoticeKind::Css, "css-a"));

    assert_eq!(
        state
            .visible()
            .map(|notice| notice.fingerprint.identity.as_str()),
        Some("css-a")
    );
}

#[test]
fn old_dismissal_does_not_hide_a_failure_after_an_intervening_fingerprint() {
    let mut state = ReloadNoticeState::default();
    state.set(notice(ReloadNoticeKind::Config, "config-a"));
    state.dismiss_visible();
    state.set(notice(ReloadNoticeKind::Config, "config-b"));
    state.dismiss_visible();

    state.set(notice(ReloadNoticeKind::Config, "config-a"));

    assert_eq!(
        state
            .visible()
            .map(|notice| notice.fingerprint.identity.as_str()),
        Some("config-a")
    );
}

#[test]
fn theme_compatibility_notice_waits_behind_failures_and_returns_after_recovery() {
    let mut state = ReloadNoticeState::default();
    state.set(notice(
        ReloadNoticeKind::ThemeCompatibility,
        "compatibility-a",
    ));
    state.set(notice(ReloadNoticeKind::Css, "css-a"));
    state.set(notice(ReloadNoticeKind::Config, "config-a"));

    assert_eq!(
        state.visible().map(|notice| notice.fingerprint.kind),
        Some(ReloadNoticeKind::Config)
    );
    state.clear(ReloadNoticeKind::Config);
    assert_eq!(
        state.visible().map(|notice| notice.fingerprint.kind),
        Some(ReloadNoticeKind::Css)
    );
    state.clear(ReloadNoticeKind::Css);
    assert_eq!(
        state.visible().map(|notice| notice.fingerprint.kind),
        Some(ReloadNoticeKind::ThemeCompatibility)
    );
}

#[test]
fn generic_dismissal_does_not_discard_a_theme_compatibility_choice() {
    let mut state = ReloadNoticeState::default();
    state.set(notice(
        ReloadNoticeKind::ThemeCompatibility,
        "compatibility-a",
    ));

    state.dismiss_visible();

    assert_eq!(
        state.visible().map(|notice| notice.fingerprint.kind),
        Some(ReloadNoticeKind::ThemeCompatibility)
    );
}
