use super::{AttributionClass, NotificationAttribution};

#[test]
fn associated_identity_keeps_presentation_and_grouping_fields_separate() {
    let attribution = NotificationAttribution::associated(
        "Signal",
        "org.signal.Signal",
        "org.signal.Signal",
        "/usr/bin/signal-desktop",
        AttributionClass::SystemAssociated,
        false,
        "desktop:org.signal.Signal".to_string(),
    );

    assert_eq!(attribution.display_name, "Signal");
    assert_eq!(attribution.desktop_id, "org.signal.Signal");
    assert_eq!(attribution.class, AttributionClass::SystemAssociated);
    assert!(!attribution.has_warning());
}

#[test]
fn conflict_diagnostics_do_not_enter_the_primary_display_name() {
    let attribution = NotificationAttribution::conflict(
        "KeePassXC",
        "source /tmp/keepassxc",
        "executable:1:2".to_string(),
    );

    assert_eq!(attribution.display_name, "Unknown application");
    assert!(attribution.source_label.contains("Claims to be KeePassXC"));
    assert!(!attribution.display_name.contains("unverified claim"));
    assert!(attribution.has_warning());
}

#[test]
fn trusted_relay_keeps_the_callers_label_without_granting_association() {
    let attribution = NotificationAttribution::trusted_relay(
        "Screenshot",
        "Sent via /usr/bin/notify-send",
        false,
        "relay:1:2:screenshot".to_string(),
    );

    assert_eq!(attribution.display_name, "Screenshot");
    assert_eq!(attribution.class, AttributionClass::TrustedRelay);
    assert!(!attribution.has_warning());
}

#[test]
fn unknown_sender_keeps_bounded_presentation_without_gaining_association() {
    let attribution = NotificationAttribution::unknown(
        "Local helper",
        "Source: /opt/local-helper",
        "executable:7:9:localhelper".to_string(),
    );

    assert_eq!(attribution.display_name, "Local helper");
    assert_eq!(attribution.source_label, "Source: /opt/local-helper");
    assert_eq!(attribution.class, AttributionClass::Unknown);
    assert_eq!(attribution.group_key, "executable:7:9:localhelper");
    assert!(!attribution.has_warning());
}
