use std::collections::{HashMap, VecDeque};

use super::{build_reconcile_plan_with_icon_refresh, desired_seed_popups};
use crate::ui::UiState;
use gtk::prelude::*;
use unixnotis_core::{
    Action, Config, ControlState, NotificationImage, NotificationView, ThemePaths, Urgency,
};
use unixnotis_ui::css::CssManager;

fn make_view(id: u32, urgency: Urgency, summary: &str) -> NotificationView {
    NotificationView {
        id,
        generation: u64::from(id),
        app_name: "Test".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: summary.to_string(),
        body: "body".to_string(),
        actions: vec![Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        }],
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: urgency as u8,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    }
}

#[test]
fn visible_generations_are_not_recreated_from_a_reconnect_seed() {
    let mut notification = make_view(10, Urgency::Normal, "already shown");
    notification.popup_decision.delivery_stage = unixnotis_core::PopupDeliveryStage::Visible;

    let desired = desired_seed_popups(vec![notification], &ControlState::default());

    assert!(desired.is_empty());
}

#[test]
fn desired_seed_clears_all_popups_when_inhibited() {
    let state = ControlState {
        inhibited: true,
        ..ControlState::default()
    };

    let desired = desired_seed_popups(vec![make_view(1, Urgency::Critical, "critical")], &state);

    assert!(desired.is_empty());
}

#[test]
fn desired_seed_keeps_only_critical_popups_during_dnd() {
    let state = ControlState {
        dnd_enabled: true,
        ..ControlState::default()
    };

    let desired = desired_seed_popups(
        vec![
            make_view(1, Urgency::Normal, "normal"),
            make_view(2, Urgency::Critical, "critical"),
        ],
        &state,
    );

    assert_eq!(desired.len(), 1);
    assert_eq!(desired[0].id, 2);
}

#[test]
fn reconcile_plan_removes_missing_rows_and_updates_changed_payloads() {
    let mut local = HashMap::new();
    local.insert(5, make_view(5, Urgency::Normal, "old"));
    local.insert(7, make_view(7, Urgency::Normal, "stale"));
    let local_order = VecDeque::from([7, 5]);
    let desired = vec![make_view(5, Urgency::Normal, "new")];

    let plan = build_reconcile_plan_with_icon_refresh(&local, &local_order, &desired, false);

    assert_eq!(plan.stale_ids, vec![7]);
    assert_eq!(plan.updates.len(), 1);
    assert_eq!(plan.updates[0].summary, "new");
    assert_eq!(plan.desired_order, VecDeque::from([5]));
}

#[test]
fn reconcile_plan_preserves_unchanged_rows_without_rebuild() {
    let mut local = HashMap::new();
    local.insert(1, make_view(1, Urgency::Normal, "keep"));
    let local_order = VecDeque::from([1]);
    let desired = vec![make_view(1, Urgency::Normal, "keep")];

    let plan = build_reconcile_plan_with_icon_refresh(&local, &local_order, &desired, false);

    assert!(plan.stale_ids.is_empty());
    assert!(plan.updates.is_empty());
    assert_eq!(plan.desired_order, VecDeque::from([1]));
}

#[test]
fn reconcile_plan_refreshes_unchanged_rows_when_icon_sources_changed() {
    let mut local = HashMap::new();
    local.insert(1, make_view(1, Urgency::Normal, "keep"));
    let local_order = VecDeque::from([1]);
    let desired = vec![make_view(1, Urgency::Normal, "keep")];

    let plan = build_reconcile_plan_with_icon_refresh(&local, &local_order, &desired, true);

    assert_eq!(plan.updates.len(), 1);
    assert_eq!(plan.updates[0].id, 1);
}

#[gtk::test]
fn reconcile_seed_rebuilds_an_unchanged_row_after_icon_source_invalidation() {
    let mut state = popup_state("org.unixnotis.PopupReconcileIconSources");
    let notification = make_view(30, Urgency::Normal, "unchanged");

    state.add_popup(notification.clone());
    let old_root = state
        .popups
        .get(&notification.id)
        .and_then(|entry| entry.root.clone())
        .expect("seed fixture should materialize a visible row");

    state.icon_source_generation = 1;
    state.reconcile_seed(vec![notification]);

    let new_root = state
        .popups
        .get(&30)
        .and_then(|entry| entry.root.clone())
        .expect("reconciled row should remain materialized");
    assert_ne!(old_root, new_root);
}

#[gtk::test]
fn reconcile_seed_refreshes_unchanged_rows_when_sources_are_dirty() {
    let mut state = popup_state("org.unixnotis.PopupReconcileDirtySources");
    let notification = make_view(31, Urgency::Normal, "unchanged");

    state.add_popup(notification.clone());
    let old_root = state
        .popups
        .get(&notification.id)
        .and_then(|entry| entry.root.clone())
        .expect("seed fixture should materialize a visible row");

    state.icon_sources_dirty.set(true);
    state.reconcile_seed(vec![notification]);

    let new_root = state
        .popups
        .get(&31)
        .and_then(|entry| entry.root.clone())
        .expect("reconciled row should remain materialized");
    assert_ne!(old_root, new_root);
}

#[gtk::test]
fn reconcile_seed_refreshes_visible_rows_once_and_advances_queued_rows() {
    let mut state = popup_state("org.unixnotis.PopupReconcileQueuedIconSources");
    let visible = make_view(40, Urgency::Normal, "visible");
    let queued = make_view(41, Urgency::Normal, "queued");

    state.add_popup(visible);
    state.add_popup(queued);

    let visible_id = state
        .popups
        .iter()
        .find_map(|(id, entry)| entry.is_materialized().then_some(*id))
        .expect("one row should be materialized");
    let queued_id = state
        .popups
        .iter()
        .find_map(|(id, entry)| (!entry.is_materialized()).then_some(*id))
        .expect("one row should remain queued");
    let old_root = state
        .popups
        .get(&visible_id)
        .and_then(|entry| entry.root.clone())
        .expect("visible row should have a root");
    let seed = state
        .popup_order
        .iter()
        .map(|id| {
            state
                .popups
                .get(id)
                .expect("seed row should exist")
                .notification
                .clone()
        })
        .collect::<Vec<_>>();

    state.icon_sources_dirty.set(true);
    state.reconcile_seed(seed.clone());

    let refreshed_root = state
        .popups
        .get(&visible_id)
        .and_then(|entry| entry.root.clone())
        .expect("refreshed row should have a root");
    assert_ne!(old_root, refreshed_root);
    assert_eq!(
        state
            .popups
            .get(&queued_id)
            .expect("queued row should remain")
            .icon_source_generation,
        state.icon_source_generation
    );

    state.reconcile_seed(seed);

    let second_root = state
        .popups
        .get(&visible_id)
        .and_then(|entry| entry.root.clone())
        .expect("visible row should remain materialized");
    assert_eq!(refreshed_root, second_root);
}

fn popup_state(application_id: &str) -> UiState {
    let app = gtk::Application::builder()
        .application_id(application_id)
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register reconcile test application");

    let mut config = Config::default();
    config.popups.max_visible = 1;
    let root = std::env::temp_dir().join("unixnotis-popup-reconcile");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(16);
    let theme_paths = ThemePaths {
        base_dir: root.clone(),
        base_css: root.join("base.css"),
        popup_css: root.join("popup.css"),
        panel_css: root.join("panel.css"),
        widgets_css: root.join("widgets.css"),
        media_css: root.join("media.css"),
    };
    let css = CssManager::new_popup(theme_paths, config.theme.clone());

    UiState::new(&app, config, root.join("config.toml"), command_tx, css)
}
