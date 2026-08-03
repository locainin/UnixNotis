use gtk::prelude::*;
use unixnotis_core::{hooks, Config, CutCorners, NotificationImage, NotificationView, Urgency};
use unixnotis_ui::{css::CssManager, CutCorner};

use super::super::UiState;
use super::support::theme_paths;

#[gtk::test]
fn popup_entry_uses_the_configured_cut_corner_primitive() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupCornerTest")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup corner test application");
    let mut config = Config::default();
    config.theme.notification_corners = CutCorners {
        top_left: 20,
        bottom_right: 14,
        ..CutCorners::default()
    };
    let corners = config.theme.notification_corners;
    let config_root = std::env::temp_dir().join("unixnotis-popup-corners");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&config_root), config.theme.clone());
    let mut state = UiState::new(
        &app,
        config,
        config_root.join("config.toml"),
        command_tx,
        css,
    );
    let notification = NotificationView {
        id: 1,
        generation: 1,
        app_name: "Demo".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "Summary".to_string(),
        body: "Body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };

    let entry = state.build_popup_entry(&notification);
    let plate = entry
        .revealer
        .and_then(|revealer| revealer.child())
        .and_downcast::<CutCorner>()
        .expect("popup revealer should contain the cut-corner primitive");
    let root = entry.root.expect("popup entry should keep its styled root");

    assert_eq!(plate.corners(), corners);
    assert_eq!(plate.child().as_ref(), Some(root.upcast_ref()));
}

#[gtk::test]
fn default_popup_entry_uses_the_native_rounded_card_without_a_clipper() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupRoundedCardTest")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup rounded-card test application");
    let config = Config::default();
    let config_root = std::env::temp_dir().join("unixnotis-popup-rounded-card");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&config_root), config.theme.clone());
    let mut state = UiState::new(
        &app,
        config,
        config_root.join("config.toml"),
        command_tx,
        css,
    );
    let notification = NotificationView {
        id: 1,
        generation: 1,
        app_name: "Demo".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "Summary".to_string(),
        body: "Body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };

    let entry = state.build_popup_entry(&notification);
    let root = entry.root.expect("popup entry should keep its styled root");
    let child = entry
        .revealer
        .and_then(|revealer| revealer.child())
        .expect("popup revealer should contain its card");

    assert_eq!(child, root.upcast::<gtk::Widget>());
}

#[gtk::test]
fn constructor_keeps_config_path_and_starts_with_empty_runtime_collections() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupStateTest")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup test application");
    let config = Config::default();
    let config_root = std::env::temp_dir().join("unixnotis-popup-state");
    let config_path = config_root.join("config.toml");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&config_root), config.theme.clone());

    let state = UiState::new(&app, config, config_path.clone(), command_tx, css);

    assert_eq!(state.config_path, config_path);
    assert!(state.popups.is_empty());
    assert!(state.popup_order.is_empty());
    assert!(state.visible_popups.is_empty());
    assert!(
        state.popup_window.is_resizable(),
        "the layer window must accept content-driven height changes"
    );
    assert_eq!(
        state.popup_window.default_size().1,
        -1,
        "popup height must use the current stack's natural request"
    );
}

#[gtk::test]
fn critical_popup_probe_builds_the_root_class_and_badge() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupCriticalProbe")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup critical probe application");
    let config = Config::default();
    let config_root = std::env::temp_dir().join("unixnotis-popup-critical-probe");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&config_root), config.theme.clone());
    let mut state = UiState::new(
        &app,
        config,
        config_root.join("config.toml"),
        command_tx,
        css,
    );
    let notification = NotificationView {
        id: 2,
        generation: 2,
        app_name: "Critical probe".to_string(),
        attribution: unixnotis_core::NotificationAttribution {
            display_name: "Critical probe".to_string(),
            ..unixnotis_core::NotificationAttribution::default()
        },
        summary: "Critical popup".to_string(),
        body: "The composed critical state must be visible".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: Urgency::Critical as u8,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };

    let root = state.build_popup_root(&notification);

    assert!(root.has_css_class(hooks::shared_state::CRITICAL));
    assert!(root.has_css_class(hooks::popup_card::HAS_SUMMARY));
    assert!(root.has_css_class(hooks::popup_card::HAS_BODY));
    assert!(!root.has_css_class(hooks::popup_card::HAS_ACTIONS));
    assert_ne!(
        root.has_css_class(hooks::popup_card::HAS_ICON),
        root.has_css_class(hooks::popup_card::NO_ICON)
    );
    assert_eq!(root.width_request(), -1);
    assert_eq!(root.height_request(), -1);
    assert!(root.hexpands());
    assert!(visible_descendant_has_class(
        root.upcast_ref(),
        hooks::urgency::BADGE
    ));
}

#[gtk::test]
fn unknown_attribution_uses_a_short_chip_without_showing_raw_provenance() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupUnverifiedProbe")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup unverified probe application");
    let config = Config::default();
    let config_root = std::env::temp_dir().join("unixnotis-popup-unverified-probe");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&config_root), config.theme.clone());
    let mut state = UiState::new(
        &app,
        config,
        config_root.join("config.toml"),
        command_tx,
        css,
    );
    let notification = NotificationView {
        id: 3,
        generation: 3,
        app_name: "Signal".to_string(),
        attribution: unixnotis_core::NotificationAttribution::unresolved(
            "Signal",
            unixnotis_core::AttributionReason::MissingSenderEvidence,
            "sender evidence unavailable",
            "unknown:signal".to_string(),
        ),
        summary: "John Doe".to_string(),
        body: "Are you free later?".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: Urgency::Normal as u8,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };

    let root = state.build_popup_root(&notification);

    assert!(root.has_css_class("unresolved"));
    assert!(root.has_css_class("utility"));
    assert!(visible_descendant_has_text(root.upcast_ref(), "Unverified"));
    assert!(!visible_descendant_has_text(
        root.upcast_ref(),
        "sender evidence unavailable"
    ));
    assert!(visible_descendant_has_text(
        root.upcast_ref(),
        "Identity could not be verified"
    ));
}

#[gtk::test]
fn conflicting_attribution_keeps_message_layout_and_uses_suspicious_chip() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupSuspiciousProbe")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup suspicious probe application");
    let config = Config::default();
    let config_root = std::env::temp_dir().join("unixnotis-popup-suspicious-probe");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&config_root), config.theme.clone());
    let mut state = UiState::new(
        &app,
        config,
        config_root.join("config.toml"),
        command_tx,
        css,
    );
    let notification = NotificationView {
        id: 4,
        generation: 4,
        app_name: "Signal".to_string(),
        attribution: unixnotis_core::NotificationAttribution::conflict(
            "Signal",
            "org.signal.Signal",
            unixnotis_core::AttributionReason::ExecutableMismatch,
            "application claim mismatch; source /tmp/fake",
            "conflict:signal".to_string(),
        ),
        summary: "John Doe".to_string(),
        body: "Are you free later?".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: Urgency::Normal as u8,
        category: "im.received".to_string(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };

    let root = state.build_popup_root(&notification);

    assert!(root.has_css_class("communication"));
    assert!(root.has_css_class("conflict"));
    assert!(visible_descendant_has_text(root.upcast_ref(), "Suspicious"));
    assert!(visible_descendant_has_text(
        root.upcast_ref(),
        "Claimed app: Signal"
    ));
    assert!(!visible_descendant_has_text(
        root.upcast_ref(),
        "application claim mismatch; source /tmp/fake"
    ));
}

#[gtk::test]
fn notify_send_claim_uses_one_command_line_avatar_without_signal_branding() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupRelayProbe")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup relay probe application");
    let config = Config::default();
    let config_root = std::env::temp_dir().join("unixnotis-popup-relay-probe");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&config_root), config.theme.clone());
    let mut state = UiState::new(
        &app,
        config,
        config_root.join("config.toml"),
        command_tx,
        css,
    );
    let mut notification = NotificationView {
        id: 5,
        generation: 5,
        app_name: "Signal".to_string(),
        attribution: unixnotis_core::NotificationAttribution::relay(
            "Signal",
            "Sent via /usr/bin/notify-send",
            "relay:notify-send:signal".to_string(),
        ),
        summary: "John Doe".to_string(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: Urgency::Normal as u8,
        category: "im.received".to_string(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };
    notification.image.badge_icon = "signal-desktop".to_string();

    let root = state.build_popup_root(&notification);

    assert!(root.has_css_class("relay"));
    assert!(root.has_css_class("communication"));
    assert!(!root.has_css_class("conflict"));
    assert!(visible_descendant_has_text(
        root.upcast_ref(),
        "Command-line notification"
    ));
    assert!(visible_descendant_has_text(
        root.upcast_ref(),
        "App label: Signal"
    ));
    assert_eq!(
        visible_descendant_class_count(root.upcast_ref(), "unixnotis-identity-avatar"),
        1
    );
    assert!(!visible_descendant_has_class(
        root.upcast_ref(),
        "unixnotis-popup-content-image"
    ));
    let close = descendant_with_class(root.upcast_ref(), "unixnotis-popup-close")
        .expect("overlay close control");
    assert!(close
        .parent()
        .is_some_and(|parent| parent.is::<gtk::Overlay>()));
}

fn visible_descendant_has_class(widget: &gtk::Widget, class_name: &str) -> bool {
    let mut child = widget.first_child();
    while let Some(current) = child {
        if current.get_visible() && current.has_css_class(class_name) {
            return true;
        }
        if visible_descendant_has_class(&current, class_name) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}

fn visible_descendant_has_text(widget: &gtk::Widget, expected: &str) -> bool {
    let mut child = widget.first_child();
    while let Some(current) = child {
        if current
            .downcast_ref::<gtk::Label>()
            .is_some_and(|label| label.get_visible() && label.text() == expected)
        {
            return true;
        }
        if visible_descendant_has_text(&current, expected) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}

fn visible_descendant_class_count(widget: &gtk::Widget, class_name: &str) -> usize {
    let mut count = 0;
    let mut child = widget.first_child();
    while let Some(current) = child {
        if current.get_visible() && current.has_css_class(class_name) {
            count += 1;
        }
        count += visible_descendant_class_count(&current, class_name);
        child = current.next_sibling();
    }
    count
}

fn descendant_with_class(widget: &gtk::Widget, class_name: &str) -> Option<gtk::Widget> {
    let mut child = widget.first_child();
    while let Some(current) = child {
        if current.has_css_class(class_name) {
            return Some(current);
        }
        if let Some(found) = descendant_with_class(&current, class_name) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}
