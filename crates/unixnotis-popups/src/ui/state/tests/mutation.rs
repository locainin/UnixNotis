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

#[gtk::test]
fn popup_widget_tree_keeps_one_identity_grid_and_overlay_close_control() {
    let mut state = popup_state("org.unixnotis.PopupWidgetTree");
    let mut relayed = notification(12, 1, "Build finished");
    relayed.attribution = unixnotis_core::NotificationAttribution::relay(
        "Builder",
        "Sent through /usr/bin/notify-send",
        "relay:notify-send:builder".to_string(),
    );
    let root = state.build_popup_root(&relayed);
    let overlay = root
        .first_child()
        .and_downcast::<gtk::Overlay>()
        .expect("popup root should contain one overlay");
    let content = overlay
        .child()
        .and_downcast::<gtk::Box>()
        .expect("overlay should own the measured popup content");
    let grid = content
        .first_child()
        .and_downcast::<gtk::Grid>()
        .expect("popup content should start with the identity grid");

    assert!(grid.has_css_class("unixnotis-popup-content-grid"));
    assert_eq!(grid.column_spacing(), 10);
    assert_eq!(grid.row_spacing(), 4);
    assert_eq!(
        grid.property::<gtk::AccessibleRole>("accessible-role"),
        gtk::AccessibleRole::Group
    );
    assert_eq!(
        descendant_class_count(root.upcast_ref(), "unixnotis-identity-avatar"),
        1,
        "one provenance-controlled avatar must own application identity"
    );
    assert!(descendant_has_text(
        root.upcast_ref(),
        "Command-line notification"
    ));
    assert!(descendant_has_text(root.upcast_ref(), "App label: Builder"));
    assert!(!descendant_has_class(
        content.upcast_ref(),
        "unixnotis-popup-close"
    ));
    assert!(descendant_has_class(
        overlay.upcast_ref(),
        "unixnotis-popup-close"
    ));
}

#[gtk::test]
fn visible_popup_materialization_and_rebuild_replace_the_exact_widget_generation() {
    let (mut state, mut command_rx) =
        popup_state_with_commands("org.unixnotis.PopupMaterialization", 1);
    let original = notification(21, 1, "original");

    state.add_popup(original.clone());
    let original_entry = state
        .popups
        .get(&original.id)
        .expect("visible popup should be stored");
    assert!(original_entry.is_materialized());
    assert_eq!(state.visible_popups, vec![original.id]);
    let original_root = original_entry
        .root
        .clone()
        .expect("visible popup should have a root");
    assert!(original_root.is_visible());
    assert_materialized_and_visible_commands(&mut command_rx, original.key());

    let replacement = notification(21, 2, "replacement");
    state.update_popup(replacement.clone(), true);
    let replacement_root = state
        .popups
        .get(&replacement.id)
        .and_then(|entry| entry.root.clone())
        .expect("replacement popup should have a root");
    assert_ne!(original_root, replacement_root);
    assert!(descendant_has_text(
        replacement_root.upcast_ref(),
        "replacement"
    ));
    assert_materialized_and_visible_commands(&mut command_rx, replacement.key());
}

#[gtk::test]
fn visible_popup_callbacks_report_each_generation_only_once() {
    let (mut state, mut command_rx) =
        popup_state_with_commands("org.unixnotis.PopupVisibleOnce", 1);
    let original = notification(24, 1, "original");
    state.add_popup(original.clone());
    assert_materialized_and_visible_commands(&mut command_rx, original.key());

    let entry = state
        .popups
        .get(&original.id)
        .expect("visible popup should be stored");
    let revealer = entry
        .revealer
        .as_ref()
        .expect("visible popup should have a revealer");
    let visibility = entry
        .visibility
        .as_ref()
        .expect("visible popup should retain its visibility binding");
    visibility.report_if_visible(revealer, &state.popup_window, &state.command_tx);
    visibility.report_if_visible(revealer, &state.popup_window, &state.command_tx);
    assert!(
        command_rx.try_recv().is_err(),
        "duplicate map and reveal callbacks must not send another acknowledgement"
    );

    let replacement = notification(24, 2, "replacement");
    state.update_popup(replacement.clone(), true);
    assert_materialized_and_visible_commands(&mut command_rx, replacement.key());
    assert!(
        command_rx.try_recv().is_err(),
        "one replacement generation should produce one visibility acknowledgement"
    );
}

#[gtk::test]
fn mapped_window_does_not_acknowledge_an_unrevealed_popup_row() {
    let (mut state, mut command_rx) = popup_state_with_commands("org.unixnotis.PopupHiddenRow", 1);
    let visible = notification(25, 1, "visible");
    state.add_popup(visible.clone());
    assert_materialized_and_visible_commands(&mut command_rx, visible.key());
    assert!(state.popup_window.is_mapped());

    let hidden_revealer = gtk::Revealer::new();
    hidden_revealer.set_child(Some(&gtk::Label::new(Some("hidden"))));
    hidden_revealer.set_reveal_child(false);
    let hidden_key = NotificationKey {
        id: 26,
        generation: 1,
    };
    let visibility = crate::ui::entry::PopupVisibilityBinding::new(hidden_key);

    visibility.report_if_visible(&hidden_revealer, &state.popup_window, &state.command_tx);

    assert!(
        command_rx.try_recv().is_err(),
        "a mapped window cannot make an unrevealed row visible"
    );
}

fn assert_materialized_and_visible_commands(
    command_rx: &mut tokio::sync::mpsc::Receiver<crate::dbus::UiCommand>,
    expected: NotificationKey,
) {
    match command_rx
        .try_recv()
        .expect("materialization acknowledgement")
    {
        crate::dbus::UiCommand::Materialized(notification) => {
            assert_eq!(notification, expected);
        }
        command => panic!("unexpected command: {command:?}"),
    }
    match command_rx.try_recv().expect("visibility acknowledgement") {
        crate::dbus::UiCommand::Visible(notification) => {
            assert_eq!(notification, expected);
        }
        command => panic!("unexpected command: {command:?}"),
    }
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

fn descendant_class_count(widget: &gtk::Widget, class_name: &str) -> usize {
    let own = usize::from(widget.has_css_class(class_name));
    let mut count = own;
    let mut child = widget.first_child();
    while let Some(current) = child {
        count += descendant_class_count(&current, class_name);
        child = current.next_sibling();
    }
    count
}

fn descendant_has_text(widget: &gtk::Widget, expected: &str) -> bool {
    if widget
        .downcast_ref::<gtk::Label>()
        .is_some_and(|label| label.text().as_str() == expected)
    {
        return true;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if descendant_has_text(&current, expected) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}

fn popup_state(application_id: &str) -> UiState {
    popup_state_with_commands(application_id, 0).0
}

fn popup_state_with_commands(
    application_id: &str,
    max_visible: usize,
) -> (UiState, tokio::sync::mpsc::Receiver<crate::dbus::UiCommand>) {
    let app = gtk::Application::builder()
        .application_id(application_id)
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup mutation application");
    let mut config = Config::default();
    config.popups.max_visible = max_visible;
    let root = std::env::temp_dir().join("unixnotis-popup-mutation");
    let (command_tx, command_rx) = tokio::sync::mpsc::channel(4);
    let css = CssManager::new_popup(theme_paths(&root), config.theme.clone());

    (
        UiState::new(&app, config, root.join("config.toml"), command_tx, css),
        command_rx,
    )
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
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
    }
}
