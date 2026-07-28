use unixnotis_core::{Action, AttributionClass, NotificationAttribution};

use super::super::{PopupKind, PopupTrustPresentation};
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
        let trust = PopupTrustPresentation::for_notification(&view);

        assert_eq!(
            PopupKind::for_notification(&view, trust.level),
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
        let trust = PopupTrustPresentation::for_notification(&view);

        assert_eq!(
            PopupKind::for_notification(&view, trust.level),
            PopupKind::Utility,
            "{category:?} should use the utility layout"
        );
    }
}

#[test]
fn suspicious_provenance_overrides_a_communication_category() {
    let mut view = notification();
    view.category = "im.received".to_string();
    view.attribution = NotificationAttribution::associated(
        "Unknown application",
        "",
        "dialog-warning-symbolic",
        "Claims to be Signal; source /tmp/fake",
        AttributionClass::Conflict,
        true,
        "conflict:signal".to_string(),
    );
    let trust = PopupTrustPresentation::for_notification(&view);

    assert_eq!(
        PopupKind::for_notification(&view, trust.level),
        PopupKind::Warning
    );
}

#[test]
fn either_reply_contract_selects_the_communication_layout() {
    let mut metadata_reply = notification();
    metadata_reply.inline_reply.available = true;
    let trust = PopupTrustPresentation::for_notification(&metadata_reply);
    assert_eq!(
        PopupKind::for_notification(&metadata_reply, trust.level),
        PopupKind::Communication
    );

    let mut action_reply = notification();
    action_reply.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    let trust = PopupTrustPresentation::for_notification(&action_reply);
    assert_eq!(
        PopupKind::for_notification(&action_reply, trust.level),
        PopupKind::Communication
    );
}

#[test]
fn each_popup_kind_keeps_its_intended_action_budget() {
    assert_eq!(PopupKind::Communication.action_limit(), 3);
    assert_eq!(PopupKind::Utility.action_limit(), 1);
    assert_eq!(PopupKind::Warning.action_limit(), 1);
}
