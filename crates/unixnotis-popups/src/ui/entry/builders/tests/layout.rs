use super::{build_popup_grid, popup_accessible_label, PopupLayout};
use crate::ui::entry::presentation::{PopupEntryViewModel, PopupKind, ReplyPresentation};
use crate::ui::UiState;
use gtk::prelude::*;
use unixnotis_core::{Config, NotificationImage, NotificationView, ThemePaths};
use unixnotis_ui::css::CssManager;
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
    view.secondary_claim = Some("Claimed app: Example Chat".to_string());
    view.badge = BadgePresentation::SuspiciousApplication;
    view.body = Some("Hey, did this go through?".to_string());
    view.trust.level = TrustLevel::Conflict;
    view.trust.short_label = Some("Suspicious".to_string());

    assert_eq!(
        popup_accessible_label(&view),
        "Unknown application. Suspicious. Claimed app: Example Chat. Build finished. \
         Hey, did this go through?"
    );
}

#[gtk::test]
fn conversation_avatar_occupies_left_grid_column_across_message_rows() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupAvatarGrid")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register avatar grid application");
    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-avatar-grid");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());
    let mut state = UiState::new(&app, config, root.join("config.toml"), command_tx, css);
    let notification = conversation_notification();
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);

    let rendered = build_popup_grid(
        &mut state,
        &notification,
        &view,
        PopupLayout {
            css_class: "unixnotis-popup-communication-content",
            body_lines: 5,
            show_reply_note: true,
        },
    );

    let avatar = rendered
        .widget
        .child_at(0, 0)
        .and_downcast::<gtk::Box>()
        .expect("left grid cell should contain the identity avatar");
    let avatar_second_row = rendered
        .widget
        .child_at(0, 1)
        .expect("avatar should span the message row");
    let icon = avatar
        .first_child()
        .and_downcast::<gtk::Image>()
        .expect("avatar slot should contain one image");

    assert_eq!(avatar_second_row, avatar.upcast::<gtk::Widget>());
    assert!(icon.has_css_class("unixnotis-popup-conversation-avatar"));
    assert!(rendered.widget.child_at(1, 1).is_some());
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

fn conversation_notification() -> NotificationView {
    let mut notification = NotificationView {
        id: 7,
        generation: 1,
        app_name: "Example Chat".to_string(),
        attribution: unixnotis_core::NotificationAttribution::unresolved(
            "Example Chat",
            unixnotis_core::AttributionReason::MissingSenderEvidence,
            "no sender evidence",
            "claim:example-chat".to_string(),
        ),
        summary: "PV2 Rivera in Tel Aviv 2026".to_string(),
        body: "10 eps I heard ts tuff asf".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        category: "im.received".to_string(),
        is_transient: false,
        received_at_unix_seconds: 1_000,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = unixnotis_core::ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![1, 2, 3, 255],
    };
    notification
}

fn theme_paths(root: &std::path::Path) -> ThemePaths {
    ThemePaths {
        base_dir: root.to_path_buf(),
        base_css: root.join("base.css"),
        popup_css: root.join("popup.css"),
        panel_css: root.join("panel.css"),
        widgets_css: root.join("widgets.css"),
        media_css: root.join("media.css"),
    }
}
