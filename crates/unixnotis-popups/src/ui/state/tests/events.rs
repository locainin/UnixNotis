use gtk::prelude::*;
use unixnotis_core::{
    render_default_config_toml, Config, ControlState, NotificationImage, NotificationView,
    PopupGateState,
};
use unixnotis_ui::css::CssManager;

use super::super::{events::apply_popup_gate, UiState};
use super::support::theme_paths;
use crate::dbus::UiEvent;

#[test]
fn popup_gate_update_changes_policy_without_replacing_runtime_counts() {
    let mut state = ControlState {
        dnd_enabled: false,
        dnd_expires_at: 0,
        inhibited: false,
        history_count: 42,
        inhibitor_count: 3,
    };

    apply_popup_gate(
        &mut state,
        PopupGateState {
            dnd_enabled: true,
            inhibited: true,
        },
    );

    assert!(state.dnd_enabled);
    assert!(state.inhibited);
    assert_eq!(state.history_count, 42);
    assert_eq!(state.inhibitor_count, 3);
}

#[test]
fn popup_gate_update_can_restore_normal_popup_policy() {
    let mut state = ControlState {
        dnd_enabled: true,
        inhibited: true,
        ..ControlState::default()
    };

    apply_popup_gate(
        &mut state,
        PopupGateState {
            dnd_enabled: false,
            inhibited: false,
        },
    );

    assert!(!state.dnd_enabled);
    assert!(!state.inhibited);
}

#[gtk::test]
fn config_reload_disabling_hover_pause_resumes_an_existing_timer() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupHoverReloadEvent")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup config-reload application");
    let config_root = tempfile::tempdir().expect("create popup config fixture");
    let config_path = config_root.path().join("config.toml");
    let mut initial = Config::default();
    initial.popups.max_visible = 1;
    let mut reloaded = initial.clone();
    reloaded.popups.pause_on_hover = false;
    let rendered = render_default_config_toml(&reloaded).expect("render popup config fixture");
    std::fs::write(&config_path, rendered).expect("write popup config fixture");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let css = CssManager::new_popup(theme_paths(config_root.path()), initial.theme.clone());
    let mut state = UiState::new(&app, initial, config_path, command_tx, css);
    let (event_tx, _event_rx) = async_channel::bounded(4);
    state.set_popup_event_sender(event_tx);
    let notification = hover_notification();
    state.add_popup(notification.clone());
    state.handle_event(UiEvent::PopupHoverChanged(notification.key(), true));
    assert!(state.popups[&notification.id].hide_timer_is_paused());

    state.handle_event(UiEvent::ConfigReload);

    assert!(!state.config.popups.pause_on_hover);
    assert!(!state.popups[&notification.id].hide_timer_is_paused());
    state.remove_popup_if_generation(notification.key());
}

fn hover_notification() -> NotificationView {
    NotificationView {
        id: 41,
        generation: 1,
        app_name: "Example".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "Reload hover policy".to_string(),
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
        popup_hide_after_ms: 5_000,
    }
}
