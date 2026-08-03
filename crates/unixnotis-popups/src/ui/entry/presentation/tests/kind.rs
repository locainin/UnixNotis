use unixnotis_core::{Action, AttributionReason, NotificationAttribution};

use super::super::PopupKind;
use super::support::notification;

#[test]
fn standard_communication_category_classes_select_the_communication_layout() {
    for category in [
        "call.incoming",
        "email.arrived",
        "im.received",
        "presence.online",
    ] {
        let mut view = notification();
        view.category = category.to_string();
        assert_eq!(
            PopupKind::for_notification(&view),
            PopupKind::Communication,
            "{category} should use the communication layout"
        );
    }
}

#[test]
fn utility_categories_and_missing_categories_select_the_compact_layout() {
    for category in ["", "device.added", "network.connected", "transfer.complete"] {
        let mut view = notification();
        view.category = category.to_string();
        assert_eq!(
            PopupKind::for_notification(&view),
            PopupKind::Utility,
            "{category:?} should use the utility layout"
        );
    }
}

#[test]
fn suspicious_provenance_preserves_the_communication_category() {
    let mut view = notification();
    view.category = "im.received".to_string();
    view.attribution = NotificationAttribution::conflict(
        "Example Chat",
        "org.example.Chat",
        AttributionReason::ExecutableMismatch,
        "source /tmp/fake",
        "conflict:example-chat".to_string(),
    );

    assert_eq!(PopupKind::for_notification(&view), PopupKind::Communication);
}

#[test]
fn either_reply_contract_selects_the_communication_layout() {
    let mut metadata_reply = notification();
    metadata_reply.inline_reply.available = true;
    assert_eq!(
        PopupKind::for_notification(&metadata_reply),
        PopupKind::Communication
    );

    let mut action_reply = notification();
    action_reply.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    assert_eq!(
        PopupKind::for_notification(&action_reply),
        PopupKind::Communication
    );
}

#[test]
fn each_popup_kind_keeps_its_intended_action_budget() {
    assert_eq!(PopupKind::Communication.action_limit(), 2);
    assert_eq!(PopupKind::Utility.action_limit(), 2);
    assert_eq!(PopupKind::Media.action_limit(), 2);
}
