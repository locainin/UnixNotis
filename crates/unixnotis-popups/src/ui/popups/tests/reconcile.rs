use std::collections::{HashMap, VecDeque};

use super::{build_reconcile_plan, desired_seed_popups};
use unixnotis_core::{Action, ControlState, NotificationImage, NotificationView, Urgency};

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
        is_transient: false,
        image: NotificationImage::default(),
    }
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

    let plan = build_reconcile_plan(&local, &local_order, &desired);

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

    let plan = build_reconcile_plan(&local, &local_order, &desired);

    assert!(plan.stale_ids.is_empty());
    assert!(plan.updates.is_empty());
    assert_eq!(plan.desired_order, VecDeque::from([1]));
}
