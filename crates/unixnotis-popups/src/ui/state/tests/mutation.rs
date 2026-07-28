use gtk::prelude::*;
use unixnotis_core::{
    hooks, CloseReason, Config, ImageData, NotificationImage, NotificationKey, NotificationView,
};
use unixnotis_ui::css::CssManager;

use super::super::UiState;
use super::support::theme_paths;
use crate::dbus::UiEvent;

#[gtk::test]
fn popup_events_preserve_newest_generation_and_exact_close_identity() {
    let mut state = popup_state("org.unixnotis.PopupMutationEvents");
    let original = notification(7, 1, "original");

    state.handle_event(UiEvent::NotificationAdded(original, true));
    assert_eq!(
        state
            .popups
            .get(&7)
            .expect("original popup should be visible")
            .notification
            .summary,
        "original"
    );

    let duplicate = notification(7, 1, "duplicate");
    // Equal generations cannot replace the payload already accepted by the UI
    state.handle_event(UiEvent::NotificationAdded(duplicate, true));
    assert_eq!(
        state
            .popups
            .get(&7)
            .expect("equal generation should preserve the popup")
            .notification
            .summary,
        "original"
    );

    let replacement = notification(7, 2, "replacement");
    state.handle_event(UiEvent::NotificationUpdated(replacement, true));
    assert_eq!(
        state
            .popups
            .get(&7)
            .expect("newer generation should replace the popup")
            .notification
            .summary,
        "replacement"
    );

    // A delayed close for generation one must leave generation two visible
    state.handle_event(UiEvent::NotificationClosed(
        NotificationKey {
            id: 7,
            generation: 1,
        },
        CloseReason::Expired,
    ));
    assert!(state.popups.contains_key(&7));

    // A newer suppressed decision removes the older visible generation
    state.handle_event(UiEvent::NotificationUpdated(
        notification(7, 3, "suppressed"),
        false,
    ));
    assert!(!state.popups.contains_key(&7));

    // The next admitted generation may create the popup again
    state.handle_event(UiEvent::NotificationUpdated(
        notification(7, 4, "restored"),
        true,
    ));
    assert!(state.popups.contains_key(&7));

    state.handle_event(UiEvent::NotificationClosed(
        NotificationKey {
            id: 7,
            generation: 4,
        },
        CloseReason::Expired,
    ));
    assert!(!state.popups.contains_key(&7));
}

#[gtk::test]
fn popup_image_builders_distinguish_content_badges_and_missing_sources() {
    let mut state = popup_state("org.unixnotis.PopupMutationImages");
    let mut content = notification(8, 1, "content");
    content.category = "image.photo".to_string();
    content.image = NotificationImage {
        has_image_data: true,
        image_data: ImageData {
            width: 2,
            height: 1,
            rowstride: 8,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: vec![255; 8],
        },
        ..NotificationImage::default()
    };
    // Image categories retain real content even when the thumbnail is compact
    assert!(state.build_content_image_widget(&content).is_some());
    let content_root = state.build_popup_root(&content);
    assert!(content_root.has_css_class(hooks::popup_card::HAS_IMAGE));
    assert!(descendant_has_class(
        content_root.upcast_ref(),
        "unixnotis-popup-content-image"
    ));

    let mut missing_content = notification(9, 1, "missing");
    missing_content.attribution.badge_icon.clear();
    missing_content.attribution.desktop_id.clear();
    // Empty content and badge sources must not create placeholder image widgets
    assert!(state.build_content_image_widget(&missing_content).is_none());
    assert!(state.build_app_icon_widget(&missing_content, 20).is_none());
    let missing_root = state.build_popup_root(&missing_content);
    assert!(!missing_root.has_css_class(hooks::popup_card::HAS_IMAGE));
    assert!(!descendant_has_class(
        missing_root.upcast_ref(),
        "unixnotis-popup-content-image"
    ));

    // A daemon-selected badge remains independent from caller image content
    missing_content.attribution.badge_icon = "dialog-information".to_string();
    assert!(state.build_app_icon_widget(&missing_content, 20).is_some());
}

fn descendant_has_class(widget: &gtk::Widget, class_name: &str) -> bool {
    let mut child = widget.first_child();
    while let Some(current) = child {
        if current.has_css_class(class_name) || descendant_has_class(&current, class_name) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}

fn popup_state(application_id: &str) -> UiState {
    let app = gtk::Application::builder()
        .application_id(application_id)
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup mutation application");
    let mut config = Config::default();
    // Queued-only rows keep the state test independent of compositor animation timing
    config.popups.max_visible = 0;
    let root = std::env::temp_dir().join("unixnotis-popup-mutation");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());

    UiState::new(&app, config, root.join("config.toml"), command_tx, css)
}

fn notification(id: u32, generation: u64, summary: &str) -> NotificationView {
    NotificationView {
        id,
        generation,
        app_name: "Example".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: summary.to_string(),
        body: "Body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
    }
}
