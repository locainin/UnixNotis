//! Generic popup visual-role matrix

use super::{build_popup_grid, conversation_notification, theme_paths, PopupLayout};
use crate::ui::entry::presentation::PopupEntryViewModel;
use crate::ui::UiState;
use gtk::prelude::*;
use unixnotis_core::{
    AttributionReason, Config, IdentityAssurance, InteractionPolicies, NotificationAttribution,
};
use unixnotis_ui::css::CssManager;

#[gtk::test]
fn unresolved_conversation_avatar_stays_in_the_message_lead_slot() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupUnresolvedConversationAvatar")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register unresolved conversation avatar application");
    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-unresolved-conversation-avatar");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());
    let mut state = UiState::new(&app, config, root.join("config.toml"), command_tx, css);
    let mut notification = conversation_notification();
    notification.attribution = unixnotis_core::NotificationAttribution::unresolved(
        "Example Chat",
        unixnotis_core::AttributionReason::MissingSenderEvidence,
        "no sender evidence",
        "unknown:example-chat".to_string(),
    );
    // A claimed desktop id may brand the header without changing the unverified state
    notification.image.claimed_desktop_id = "folder".to_string();
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);

    assert_eq!(
        view.trust.level,
        unixnotis_ui::presentation::TrustLevel::Unresolved
    );
    assert_eq!(
        view.visuals.sender,
        unixnotis_ui::presentation::SenderVisualPresentation::ConversationAvatar
    );
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

    let header_row = rendered
        .widget
        .child_at(0, 0)
        .and_downcast::<gtk::Box>()
        .expect("header row should exist");
    let application_identity = header_row
        .first_child()
        .and_downcast::<gtk::Box>()
        .expect("header row should contain the application identity");
    let message_row = rendered
        .widget
        .child_at(0, 1)
        .and_downcast::<gtk::Box>()
        .expect("message row should exist");
    let conversation_avatar = message_row
        .first_child()
        .and_downcast::<gtk::Box>()
        .expect("message row should contain the conversation avatar");
    let application_icon = application_identity
        .first_child()
        .and_downcast::<gtk::Image>()
        .expect("identity slot should contain one image");
    let conversation_icon = conversation_avatar
        .first_child()
        .and_downcast::<gtk::Image>()
        .expect("conversation slot should contain one image");

    assert!(!application_icon.has_css_class("unixnotis-popup-conversation-avatar"));
    assert!(application_identity.has_css_class("unixnotis-popup-application-icon-slot"));
    assert!(application_icon.paintable().is_some());
    assert!(conversation_icon.has_css_class("unixnotis-popup-conversation-avatar"));
    assert!(!conversation_avatar.has_css_class("unixnotis-identity-avatar"));
    assert!(!conversation_avatar.has_css_class("unixnotis-popup-application-icon-slot"));
    assert!(!rendered.has_image);
    assert!(header_row.has_css_class("unixnotis-popup-header-row"));
    assert!(message_row.has_css_class("unixnotis-popup-message-row"));
}

#[gtk::test]
fn trust_state_does_not_change_conversation_avatar_geometry() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupTrustGeometry")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register trust geometry application");
    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-trust-geometry");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());
    let mut state = UiState::new(&app, config, root.join("config.toml"), command_tx, css);

    for (name, attribution) in trust_attributions() {
        let mut notification = conversation_notification();
        notification.attribution = attribution;
        let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);
        let rendered = build_popup_grid(
            &mut state,
            &notification,
            &view,
            PopupLayout {
                css_class: "unixnotis-popup-communication-content",
                body_lines: 5,
                show_reply_note: false,
            },
        );

        let header_row = rendered
            .widget
            .child_at(0, 0)
            .and_downcast::<gtk::Box>()
            .unwrap_or_else(|| panic!("missing application header for {name}"));
        let application_identity = header_row
            .first_child()
            .and_downcast::<gtk::Box>()
            .unwrap_or_else(|| panic!("missing application identity for {name}"));
        let message_row = rendered
            .widget
            .child_at(0, 1)
            .and_downcast::<gtk::Box>()
            .unwrap_or_else(|| panic!("missing message row for {name}"));
        let conversation_avatar = message_row
            .first_child()
            .and_downcast::<gtk::Box>()
            .unwrap_or_else(|| panic!("missing conversation avatar for {name}"));
        assert!(application_identity.has_css_class("unixnotis-popup-application-icon-slot"));
        assert!(!application_identity.has_css_class("unixnotis-identity-avatar"));
        assert!(conversation_avatar.has_css_class("unixnotis-popup-conversation-avatar-slot"));
        assert!(!conversation_avatar.has_css_class("unixnotis-identity-avatar"));
        assert_eq!(conversation_avatar.width_request(), 46);
        assert_eq!(conversation_avatar.height_request(), 46);
        assert!(!conversation_avatar.compute_expand(gtk::Orientation::Horizontal));
        assert!(!conversation_avatar.compute_expand(gtk::Orientation::Vertical));
        assert!(message_row.last_child().is_some());
    }
}

#[gtk::test]
fn fixed_visual_slots_do_not_consume_short_message_width() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupFixedVisualSlots")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register fixed visual slot application");
    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-fixed-visual-slots");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());
    let mut state = UiState::new(&app, config, root.join("config.toml"), command_tx, css);
    let mut notification = conversation_notification();
    notification.summary = "A".to_string();
    notification.body = "B".to_string();
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);

    let rendered = build_popup_grid(
        &mut state,
        &notification,
        &view,
        PopupLayout {
            css_class: "unixnotis-popup-communication-content",
            body_lines: 5,
            show_reply_note: false,
        },
    );

    let header_row = rendered
        .widget
        .child_at(0, 0)
        .and_downcast::<gtk::Box>()
        .expect("header row should exist");
    let application_slot = header_row
        .first_child()
        .and_downcast::<gtk::Box>()
        .expect("header should contain the application visual slot");
    let message_row = rendered
        .widget
        .child_at(0, 1)
        .and_downcast::<gtk::Box>()
        .expect("message row should exist");
    let conversation_slot = message_row
        .first_child()
        .and_downcast::<gtk::Box>()
        .expect("message row should contain the conversation visual slot");
    let message_column = message_row
        .last_child()
        .and_downcast::<gtk::Box>()
        .expect("message row should contain the message column");

    assert_eq!(application_slot.width_request(), 24);
    assert_eq!(conversation_slot.width_request(), 46);
    assert!(!application_slot.compute_expand(gtk::Orientation::Horizontal));
    assert!(!conversation_slot.compute_expand(gtk::Orientation::Horizontal));
    assert!(message_column.compute_expand(gtk::Orientation::Horizontal));
}

#[gtk::test]
fn conflict_popup_keeps_warning_badge_ahead_of_claimed_branding() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupConflictBranding")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register conflict branding application");
    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-conflict-branding");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());
    let mut state = UiState::new(&app, config, root.join("config.toml"), command_tx, css);
    let mut notification = conversation_notification();
    notification.attribution = unixnotis_core::NotificationAttribution::conflict(
        "Example Chat",
        "org.example.Chat",
        AttributionReason::ExecutableMismatch,
        "identity conflict",
        "conflict:example-chat".to_string(),
    );
    notification.image.claimed_theme_icon = "folder".to_string();
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);
    let rendered = build_popup_grid(
        &mut state,
        &notification,
        &view,
        PopupLayout {
            css_class: "unixnotis-popup-communication-content",
            body_lines: 5,
            show_reply_note: false,
        },
    );

    let header_row = rendered
        .widget
        .child_at(0, 0)
        .and_downcast::<gtk::Box>()
        .expect("conflict header row");
    let application_identity = header_row
        .first_child()
        .and_downcast::<gtk::Box>()
        .expect("conflict header identity slot");
    let icon = application_identity
        .first_child()
        .and_downcast::<gtk::Image>()
        .expect("conflict header icon");

    assert_eq!(
        icon.icon_name().as_deref(),
        Some("unixnotis-shield-warning-symbolic")
    );
    assert_eq!(
        view.trust.level,
        unixnotis_ui::presentation::TrustLevel::Conflict
    );
}

#[gtk::test]
fn conversation_avatar_and_content_image_use_separate_popup_lanes() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupVisualLanes")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register visual lane application");
    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-visual-lanes");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());
    let mut state = UiState::new(&app, config, root.join("config.toml"), command_tx, css);
    let mut notification = conversation_notification();
    notification.image.content_image = unixnotis_core::ImageData {
        width: 2,
        height: 2,
        rowstride: 8,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: [9, 8, 7, 255].repeat(4),
    };
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);

    let rendered = build_popup_grid(
        &mut state,
        &notification,
        &view,
        PopupLayout {
            css_class: "unixnotis-popup-communication-content",
            body_lines: 5,
            show_reply_note: false,
        },
    );

    let message_row = rendered
        .widget
        .child_at(0, 1)
        .and_downcast::<gtk::Box>()
        .expect("message row should exist");
    let avatar = message_row
        .first_child()
        .and_downcast::<gtk::Box>()
        .expect("conversation avatar should stay beside the message");
    assert!(avatar
        .first_child()
        .is_some_and(|child| child.has_css_class("unixnotis-popup-conversation-avatar")));

    let message = message_row
        .last_child()
        .and_downcast::<gtk::Box>()
        .expect("message column should exist");
    let content_image = message
        .last_child()
        .and_downcast::<gtk::Image>()
        .expect("content media should remain below the message");
    assert!(content_image.has_css_class("unixnotis-popup-content-image"));
    assert!(rendered.has_image);
}

#[gtk::test]
fn invalid_conversation_pixels_remove_the_popup_avatar_cell() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupInvalidConversationAvatar")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register invalid avatar application");
    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-invalid-conversation-avatar");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());
    let mut state = UiState::new(&app, config, root.join("config.toml"), command_tx, css);
    let mut notification = conversation_notification();
    notification.image.sender_visual = unixnotis_core::ImageData {
        width: 0,
        height: 0,
        data: vec![1, 2, 3, 255],
        ..unixnotis_core::ImageData::default()
    };
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);

    let rendered = build_popup_grid(
        &mut state,
        &notification,
        &view,
        PopupLayout {
            css_class: "unixnotis-popup-communication-content",
            body_lines: 5,
            show_reply_note: false,
        },
    );

    assert!(rendered.widget.child_at(0, 0).is_some());
    let message_row = rendered
        .widget
        .child_at(0, 1)
        .and_downcast::<gtk::Box>()
        .expect("message row should remain when avatar decoding fails");
    assert!(!message_row
        .first_child()
        .is_some_and(|child| child.has_css_class("unixnotis-popup-conversation-avatar-slot")));
    assert!(!rendered.has_image);
}

#[gtk::test]
fn ordinary_notifications_do_not_gain_a_second_avatar_row() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupOrdinaryNotification")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register ordinary notification application");
    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-ordinary-notification");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());
    let mut state = UiState::new(&app, config, root.join("config.toml"), command_tx, css);
    let mut notification = conversation_notification();
    notification.category.clear();
    notification.image.sender_visual_role = unixnotis_core::NotificationVisualRole::None;
    notification.image.sender_visual = unixnotis_core::ImageData::default();
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);

    let rendered = build_popup_grid(
        &mut state,
        &notification,
        &view,
        PopupLayout {
            css_class: "unixnotis-popup-utility-content",
            body_lines: 5,
            show_reply_note: false,
        },
    );

    assert!(rendered.widget.child_at(0, 0).is_some());
    let message_row = rendered
        .widget
        .child_at(0, 1)
        .and_downcast::<gtk::Box>()
        .expect("ordinary notifications still have one message row");
    assert!(!message_row
        .first_child()
        .is_some_and(|child| child.has_css_class("unixnotis-popup-conversation-avatar-slot")));
}

#[gtk::test]
fn content_only_popup_keeps_media_below_message_without_avatar_slot() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupContentOnly")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register content-only application");
    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-content-only");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());
    let mut state = UiState::new(&app, config, root.join("config.toml"), command_tx, css);
    let mut notification = conversation_notification();
    notification.image.sender_visual_role = unixnotis_core::NotificationVisualRole::None;
    notification.image.sender_visual = unixnotis_core::ImageData::default();
    notification.image.content_image = unixnotis_core::ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![8, 9, 10, 255],
    };
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);

    let rendered = build_popup_grid(
        &mut state,
        &notification,
        &view,
        PopupLayout {
            css_class: "unixnotis-popup-media-content",
            body_lines: 5,
            show_reply_note: false,
        },
    );

    assert!(rendered.widget.child_at(0, 0).is_some());
    let message_row = rendered
        .widget
        .child_at(0, 1)
        .and_downcast::<gtk::Box>()
        .expect("content-only notifications still have one message row");
    assert!(!message_row
        .first_child()
        .is_some_and(|child| child.has_css_class("unixnotis-popup-conversation-avatar-slot")));
    let message = message_row
        .last_child()
        .and_downcast::<gtk::Box>()
        .expect("message column should exist");
    let content = message
        .last_child()
        .and_downcast::<gtk::Image>()
        .expect("content image should remain in the message column");
    assert!(content.has_css_class("unixnotis-popup-content-image"));
    assert!(rendered.has_image);
}

fn trust_attributions() -> [(&'static str, NotificationAttribution); 7] {
    [
        (
            "authenticated",
            NotificationAttribution::verified(
                "Example Chat",
                "Example Chat",
                "org.example.Chat",
                "example-chat",
                AttributionReason::ExactSystemExecutable,
                "authenticated fixture",
                "verified:example-chat".to_string(),
            ),
        ),
        (
            "system-associated",
            NotificationAttribution::associated(
                "Example Chat",
                "Example Chat",
                "org.example.Chat",
                "example-chat",
                IdentityAssurance::SystemAssociated,
                InteractionPolicies::NATIVE_COMPATIBILITY,
                AttributionReason::ExactSystemExecutable,
                "system fixture",
                "associated:system:example-chat".to_string(),
            ),
        ),
        (
            "user-associated",
            NotificationAttribution::associated(
                "Example Chat",
                "Example Chat",
                "org.example.Chat",
                "example-chat",
                IdentityAssurance::UserAssociated,
                InteractionPolicies::CONFIRM_ACTIONS,
                AttributionReason::ExactUserExecutable,
                "user fixture",
                "associated:user:example-chat".to_string(),
            ),
        ),
        (
            "portal-associated",
            NotificationAttribution::associated(
                "Example Chat",
                "Example Chat",
                "org.example.Chat",
                "example-chat",
                IdentityAssurance::PortalAssociated,
                InteractionPolicies::CONFIRM_ACTIONS,
                AttributionReason::PortalAppIdAssociation,
                "portal fixture",
                "associated:portal:example-chat".to_string(),
            ),
        ),
        (
            "unresolved",
            NotificationAttribution::unresolved(
                "Example Chat",
                AttributionReason::MissingSenderEvidence,
                "unresolved fixture",
                "unknown:example-chat".to_string(),
            ),
        ),
        (
            "conflict",
            NotificationAttribution::conflict(
                "Example Chat",
                "org.example.Chat",
                AttributionReason::ExecutableMismatch,
                "conflict fixture",
                "conflict:example-chat".to_string(),
            ),
        ),
        (
            "relay",
            NotificationAttribution::relay(
                "Example Chat",
                "relay fixture",
                "relay:example-chat".to_string(),
            ),
        ),
    ]
}
