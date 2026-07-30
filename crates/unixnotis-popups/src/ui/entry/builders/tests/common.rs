use super::{
    build_action_row, build_body_label, build_close_button, build_identity_avatar,
    build_identity_header, build_reply_note, build_secondary_claim, build_title_label,
    build_urgency_badge,
};
use gtk::prelude::*;
use unixnotis_core::{
    Action, AttributionReason, InlineReply, InlineReplyPolicy, NotificationAttribution,
    NotificationImage, NotificationView,
};

use crate::dbus::UiCommand;
use crate::ui::entry::presentation::{PopupEntryViewModel, ReplyPresentation};
use crate::ui::UiState;
use unixnotis_core::{Config, ThemePaths};
use unixnotis_ui::css::CssManager;

#[gtk::test]
fn popup_critical_badge_uses_shared_hook_and_visibility() {
    let critical = build_urgency_badge(true);
    let normal = build_urgency_badge(false);

    assert!(critical.has_css_class(unixnotis_core::hooks::urgency::BADGE));
    assert_eq!(critical.text().as_str(), "!");
    assert_eq!(
        critical.tooltip_text().as_deref(),
        Some("Critical notification")
    );
    assert!(critical.get_visible());
    assert!(!normal.get_visible());
}

#[gtk::test]
fn title_and_body_builders_keep_text_classes_and_line_limits() {
    let mut view = view_model();

    let title = build_title_label(&view).expect("visible title");
    let body = build_body_label(&view, 3).expect("visible body");

    assert_eq!(title.text().as_str(), "Primary title");
    assert!(title.has_css_class("unixnotis-popup-summary"));
    assert_eq!(title.lines(), 2);
    assert_eq!(body.text().as_str(), "Supporting body");
    assert!(body.has_css_class("unixnotis-popup-body"));
    assert_eq!(body.lines(), 3);

    view.title.clear();
    view.body = None;
    assert!(build_title_label(&view).is_none());
    assert!(build_body_label(&view, 3).is_none());
}

#[gtk::test]
fn secondary_claim_stays_on_one_compact_metadata_line() {
    let mut view = view_model();
    view.secondary_claim = Some("Claimed app: Signal".to_string());

    let claim = build_secondary_claim(&view).expect("secondary claim");

    assert_eq!(claim.text().as_str(), "Claimed app: Signal");
    assert!(claim.is_single_line_mode());
    assert_eq!(claim.ellipsize(), gtk::pango::EllipsizeMode::End);
    assert!(!claim.wraps());
}

#[gtk::test]
fn reply_note_exists_only_when_the_policy_explanation_is_needed() {
    let mut view = view_model();
    assert!(build_reply_note(&view).is_none());

    view.trust.reply = ReplyPresentation::Unavailable;
    let note = build_reply_note(&view).expect("reply unavailable note");

    assert_eq!(note.text().as_str(), "Reply unavailable");
    assert!(note.has_css_class("unixnotis-popup-footer-note"));
}

#[gtk::test]
fn close_button_and_identity_header_keep_their_interaction_contracts() {
    let close = build_close_button();
    let header = build_identity_header(&view_model());

    assert!(close.has_css_class("unixnotis-popup-close"));
    assert_eq!(
        close.tooltip_text().as_deref(),
        Some("Dismiss notification")
    );
    assert!(header.identity.hexpands());
    assert_eq!(header.trailing.margin_end(), 26);
    assert_eq!(header.trailing.orientation(), gtk::Orientation::Vertical);
    assert!(header
        .trailing
        .first_child()
        .is_some_and(|child| child.has_css_class("unixnotis-popup-time")));
    assert!(header
        .trailing
        .last_child()
        .is_some_and(|child| { child.has_css_class(unixnotis_core::hooks::urgency::BADGE) }));
}

#[gtk::test]
fn action_row_dispatches_the_prepared_action_identity() {
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);
    let view = view_model_with_action();
    let notification = notification();
    let row = build_action_row(&command_tx, notification.key(), &view).expect("action row");
    let button = row
        .first_child()
        .and_downcast::<gtk::Button>()
        .expect("action button");

    button.emit_clicked();

    match command_rx.try_recv().expect("queued action command") {
        UiCommand::InvokeAction {
            notification,
            action_key,
            confirmed,
        } => {
            assert_eq!(notification.id, 41);
            assert_eq!(notification.generation, 3);
            assert_eq!(action_key, "open");
            assert!(!confirmed, "allowed actions should not claim confirmation");
        }
        command => panic!("unexpected command: {command:?}"),
    }
}

#[gtk::test]
fn confirmable_popup_action_requires_two_clicks_before_dispatch() {
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);
    let mut notification = notification();
    notification.attribution = NotificationAttribution::associated(
        "Example Chat",
        "Example Chat",
        "org.example.Chat",
        "org.example.Chat",
        unixnotis_core::IdentityAssurance::SystemAssociated,
        unixnotis_core::InteractionPolicies::NATIVE_COMPATIBILITY,
        AttributionReason::ExactSystemExecutable,
        "protected executable association",
        "associated:system-app:org.example.Chat".to_string(),
    );
    notification.actions.push(Action {
        key: "archive".to_string(),
        label: "Archive".to_string(),
    });
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);
    let row = build_action_row(&command_tx, notification.key(), &view).expect("action row");
    let button = row
        .first_child()
        .and_downcast::<gtk::Button>()
        .expect("confirmable action button");

    button.emit_clicked();
    assert_eq!(button.label().as_deref(), Some("Confirm Archive"));
    assert!(
        command_rx.try_recv().is_err(),
        "first click must not invoke a confirmable action"
    );

    std::thread::sleep(std::time::Duration::from_millis(400));
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }

    button.emit_clicked();
    assert!(matches!(
        command_rx.try_recv(),
        Ok(UiCommand::InvokeAction {
            notification,
            action_key,
            confirmed: true,
        }) if notification.id == 41
            && notification.generation == 3
            && action_key == "archive"
    ));

    button.emit_clicked();
    assert_eq!(button.label().as_deref(), Some("Confirm Archive"));
    assert!(
        command_rx.try_recv().is_err(),
        "third click must re-arm rather than dispatching"
    );
}

#[gtk::test]
fn confirmable_popup_action_stale_timer_does_not_disarm_newer_cycle() {
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let mut notification = notification();
    notification.attribution = NotificationAttribution::associated(
        "Example Chat",
        "Example Chat",
        "org.example.Chat",
        "org.example.Chat",
        unixnotis_core::IdentityAssurance::SystemAssociated,
        unixnotis_core::InteractionPolicies::NATIVE_COMPATIBILITY,
        AttributionReason::ExactSystemExecutable,
        "protected executable association",
        "associated:system-app:org.example.Chat".to_string(),
    );
    notification.actions.push(Action {
        key: "archive".to_string(),
        label: "Archive".to_string(),
    });
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);
    let row = build_action_row(&command_tx, notification.key(), &view).expect("action row");
    let button = row
        .first_child()
        .and_downcast::<gtk::Button>()
        .expect("confirmable action button");

    button.emit_clicked();
    assert_eq!(button.label().as_deref(), Some("Confirm Archive"));
    assert!(command_rx.try_recv().is_err());

    std::thread::sleep(std::time::Duration::from_millis(400));
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }

    button.emit_clicked();
    assert!(matches!(
        command_rx.try_recv(),
        Ok(UiCommand::InvokeAction {
            notification,
            action_key,
            confirmed: true,
        }) if notification.id == 41 && notification.generation == 3 && action_key == "archive"
    ));

    button.emit_clicked();
    assert_eq!(button.label().as_deref(), Some("Confirm Archive"));
    assert!(command_rx.try_recv().is_err());

    // Timer A (from first arm at t=0) fires at t=5000. We are at t=400 now.
    // Sleep 4600ms -> t=5000. Process timer A. It should NOT clear cycle B.
    std::thread::sleep(std::time::Duration::from_millis(4600));
    while context.pending() {
        context.iteration(false);
    }
    assert_eq!(button.label().as_deref(), Some("Confirm Archive"));
    assert!(command_rx.try_recv().is_err());

    // Timer B (from second arm at t=400) fires at t=5400. We are at t=5000.
    // Sleep 400ms -> t=5400. Process timer B. It SHOULD clear cycle B.
    std::thread::sleep(std::time::Duration::from_millis(400));
    while context.pending() {
        context.iteration(false);
    }
    assert_eq!(button.label().as_deref(), Some("Archive"));
    assert!(command_rx.try_recv().is_err());

    // Next click re-arms rather than invokes.
    button.emit_clicked();
    assert_eq!(button.label().as_deref(), Some("Confirm Archive"));
    assert!(command_rx.try_recv().is_err());
}

#[gtk::test]
fn extra_safe_action_builds_a_compact_overflow_menu() {
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let mut notification = notification();
    notification.actions = vec![
        Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        },
        Action {
            key: "folder".to_string(),
            label: "Open folder".to_string(),
        },
        Action {
            key: "archive".to_string(),
            label: "Archive".to_string(),
        },
        Action {
            key: "mute".to_string(),
            label: "Mute".to_string(),
        },
    ];
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);
    let row = build_action_row(&command_tx, notification.key(), &view).expect("action row");
    let menu = row
        .last_child()
        .and_downcast::<gtk::MenuButton>()
        .expect("overflow menu");

    assert_eq!(menu.icon_name().as_deref(), Some("view-more-symbolic"));
    assert!(menu.popover().is_some());
}

#[gtk::test]
fn empty_action_model_does_not_build_an_action_row() {
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    assert!(build_action_row(&command_tx, notification().key(), &view_model()).is_none());
}

#[gtk::test]
fn identity_avatar_scales_the_symbolic_glyph_inside_its_fixed_slot() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupAvatarSizing")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register avatar sizing application");
    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-avatar-sizing");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());
    let mut state = UiState::new(&app, config, root.join("config.toml"), command_tx, css);
    let mut notification = notification();
    notification.attribution = NotificationAttribution::relay(
        "Signal",
        "Sent via /usr/bin/notify-send",
        "relay:notify-send:signal".to_string(),
    );
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);

    let avatar = build_identity_avatar(&mut state, &notification, &view, 36);
    let icon = avatar
        .widget
        .first_child()
        .and_downcast::<gtk::Image>()
        .expect("avatar should contain one image");

    assert_eq!(avatar.widget.width_request(), 36);
    assert_eq!(avatar.widget.height_request(), 36);
    assert_eq!(icon.pixel_size(), 22);
    assert!(icon.hexpands());
    assert!(icon.vexpands());
}

fn view_model() -> PopupEntryViewModel {
    PopupEntryViewModel::for_notification_at(&notification(), 1_000)
}

fn view_model_with_action() -> PopupEntryViewModel {
    let mut notification = notification();
    notification.actions.push(Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    });
    PopupEntryViewModel::for_notification_at(&notification, 1_000)
}

fn notification() -> NotificationView {
    NotificationView {
        id: 41,
        generation: 3,
        app_name: "Example".to_string(),
        attribution: NotificationAttribution::verified(
            "Example",
            "Example",
            "org.example.App",
            "example-app",
            AttributionReason::ExactSystemExecutable,
            "exact system executable",
            "system-app:org.example.App".to_string(),
        ),
        summary: "Primary title".to_string(),
        body: "Supporting body".to_string(),
        actions: Vec::new(),
        inline_reply: InlineReply::default(),
        inline_reply_policy: InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 1_000,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
    }
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
