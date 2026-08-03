use super::popup_accessible_label;
use crate::ui::entry::presentation::{PopupEntryViewModel, PopupKind, ReplyPresentation};
use unixnotis_ui::presentation::{
    BadgePresentation, SenderVisualPresentation, ThumbnailKind, TrustLevel, TrustPresentation,
    VisualPresentation,
};

#[test]
fn popup_accessible_name_keeps_identity_and_message_context() {
    let mut view = view_model();

    assert_eq!(
        popup_accessible_label(&view),
        "Command-line notification. App label: Builder. Build finished"
    );

    view.title.clear();
    assert_eq!(
        popup_accessible_label(&view),
        "Command-line notification. App label: Builder"
    );
}

#[test]
fn conflict_accessible_name_includes_trust_claim_and_body() {
    let mut view = view_model();
    view.app_label = "Unknown application".to_string();
    view.secondary_claim = Some("Claimed app: Signal".to_string());
    view.badge = BadgePresentation::SuspiciousApplication;
    view.body = Some("Hey, did this go through?".to_string());
    view.trust.level = TrustLevel::Conflict;
    view.trust.short_label = Some("Suspicious".to_string());

    assert_eq!(
        popup_accessible_label(&view),
        "Unknown application. Suspicious. Claimed app: Signal. Build finished. \
         Hey, did this go through?"
    );
}

fn view_model() -> PopupEntryViewModel {
    PopupEntryViewModel {
        kind: PopupKind::Communication,
        app_label: "Command-line notification".to_string(),
        secondary_claim: Some("App label: Builder".to_string()),
        badge: BadgePresentation::CommandLine,
        timestamp_label: "now".to_string(),
        title: "Build finished".to_string(),
        body: None,
        thumbnail: ThumbnailKind::None,
        visuals: VisualPresentation {
            sender: SenderVisualPresentation::None,
            content_image: false,
        },
        default_action_key: None,
        primary_actions: Vec::new(),
        overflow_actions: Vec::new(),
        trust: TrustPresentation {
            level: TrustLevel::Relay,
            short_label: None,
            details_label: None,
            reply: ReplyPresentation::Hidden,
        },
        critical: false,
    }
}
