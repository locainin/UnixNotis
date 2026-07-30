use std::sync::Arc;

use unixnotis_core::{
    Action, AttributionReason, IdentityAssurance, InteractionPolicies,
    NotificationAttribution,
};

use crate::store::test_support::{make_notification, make_store_with_limits};

#[test]
fn active_action_target_requires_an_exact_action_on_the_live_generation() {
    let mut store = make_store_with_limits(12, 20);
    let mut notification = make_notification("action");
    notification.attribution = NotificationAttribution::verified(
        "Action source",
        "Action source",
        "org.example.ActionSource",
        "",
        AttributionReason::ExactSystemExecutable,
        "exact system executable",
        "system-app:org.example.ActionSource".to_string(),
    );
    notification.actions.push(Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    });
    let original = store.insert(notification, 0).notification;
    let id = original.id;
    let key = original.key();

    let target = store
        .active_action_target_generation(key, "open", false)
        .expect("stored action should resolve");
    assert!(Arc::ptr_eq(&target, &original));
    assert!(store
        .active_action_target_generation(key, "missing", false)
        .is_none());
    assert!(store.is_active_notification_generation(id, &original));

    let replacement = store.insert(make_notification("replacement"), id);
    assert!(replacement.replaced);
    assert!(!store.is_active_notification_generation(id, &original));
    assert!(store
        .active_action_target_generation(key, "open", false)
        .is_none());
}

#[test]
fn active_action_target_denies_every_unverified_sender_class() {
    for attribution in [
        NotificationAttribution::recognized(
            "User application",
            "User application",
            "org.example.UserApplication",
            "",
            AttributionReason::ExactUserExecutable,
            "exact user executable",
            "user-app:org.example.UserApplication".to_string(),
        ),
        NotificationAttribution::unresolved(
            "Signal",
            AttributionReason::NoDesktopCandidate,
            "source /tmp/fake",
            "unknown:signal".to_string(),
        ),
        NotificationAttribution::conflict(
            "Signal",
            "org.signal.Signal",
            AttributionReason::ExecutableMismatch,
            "source /tmp/fake",
            "conflict:signal".to_string(),
        ),
        NotificationAttribution::relay(
            "Signal",
            "trusted relay /usr/bin/notify-send",
            "relay:notify-send:signal".to_string(),
        ),
    ] {
        let mut store = make_store_with_limits(12, 20);
        let mut notification = make_notification("untrusted action");
        notification.attribution = attribution;
        notification.actions.push(Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        });
        let key = store.insert(notification, 0).notification.key();

        assert!(
            store
                .active_action_target_generation(key, "default", true)
                .is_none(),
            "weak attribution should not expose application actions"
        );
    }
}

#[test]
fn native_association_allows_default_but_requires_confirmation_for_buttons() {
    let mut store = make_store_with_limits(12, 20);
    let mut notification = make_notification("native associated actions");
    notification.attribution = NotificationAttribution::associated(
        "Example Chat",
        "Example Chat",
        "org.example.Chat",
        "org.example.Chat",
        IdentityAssurance::SystemAssociated,
        InteractionPolicies::NATIVE_COMPATIBILITY,
        AttributionReason::ExactSystemExecutable,
        "protected executable association",
        "associated:system-app:org.example.Chat".to_string(),
    );
    notification.actions = vec![
        Action {
            key: "default".to_string(),
            label: String::new(),
        },
        Action {
            key: "archive".to_string(),
            label: "Archive".to_string(),
        },
    ];
    let key = store.insert(notification, 0).notification.key();

    assert!(
        store
            .active_action_target_generation(key, "default", false)
            .is_some(),
        "native default activation should retain compatibility"
    );
    assert!(
        store
            .active_action_target_generation(key, "archive", false)
            .is_none(),
        "additional native action must reject an unconfirmed request"
    );
    assert!(
        store
            .active_action_target_generation(key, "archive", true)
            .is_some(),
        "additional native action should accept explicit trusted-UI confirmation"
    );
}

#[test]
fn portal_association_requires_confirmation_for_default_and_buttons() {
    let mut store = make_store_with_limits(12, 20);
    let mut notification = make_notification("portal associated actions");
    notification.attribution = NotificationAttribution::associated(
        "Example Portal App",
        "Example Portal App",
        "org.example.PortalApp",
        "org.example.PortalApp",
        IdentityAssurance::PortalAssociated,
        InteractionPolicies::CONFIRM_ACTIONS,
        AttributionReason::PortalAppIdAssociation,
        "portal app id without confinement provenance",
        "associated:portal-app:org.example.PortalApp".to_string(),
    );
    notification.actions = vec![
        Action {
            key: "default".to_string(),
            label: String::new(),
        },
        Action {
            key: "open".to_string(),
            label: "Open".to_string(),
        },
    ];
    let key = store.insert(notification, 0).notification.key();

    for action_key in ["default", "open"] {
        assert!(
            store
                .active_action_target_generation(key, action_key, false)
                .is_none(),
            "portal action {action_key:?} must reject an unconfirmed request"
        );
        assert!(
            store
                .active_action_target_generation(key, action_key, true)
                .is_some(),
            "portal action {action_key:?} should accept trusted-UI confirmation"
        );
    }
}

#[test]
fn active_action_target_rejects_inline_reply_even_when_confirmed() {
    let mut store = make_store_with_limits(12, 20);
    let mut notification = make_notification("inline-reply action target");
    notification.attribution = NotificationAttribution::verified(
        "Verified source",
        "Verified source",
        "org.example.Verified",
        "",
        AttributionReason::ExactSystemExecutable,
        "exact system executable",
        "system-app:org.example.Verified".to_string(),
    );
    notification.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    notification.actions.push(Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    });
    let key = store.insert(notification, 0).notification.key();

    assert!(
        store
            .active_action_target_generation(key, "inline-reply", true)
            .is_none(),
        "inline-reply must be rejected through action dispatch even with confirmed=true"
    );
    assert!(
        store
            .active_action_target_generation(key, "open", false)
            .is_some(),
        "unrelated actions must still resolve normally"
    );
}
