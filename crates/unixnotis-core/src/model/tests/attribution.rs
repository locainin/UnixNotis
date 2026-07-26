use super::{AttributionClass, InlineReplyPolicy, NotificationAttribution};
use zbus::zvariant::{serialized::Context, to_bytes, Type, LE};

#[test]
fn attribution_wire_enums_use_their_declared_one_byte_signature() {
    let context = Context::new_dbus(LE, 0);

    for (class, discriminant) in [
        (AttributionClass::SystemAssociated, 0_u8),
        (AttributionClass::PortalAssociated, 1),
        (AttributionClass::UserAssociated, 2),
        (AttributionClass::TrustedRelay, 3),
        (AttributionClass::Unknown, 4),
        (AttributionClass::Conflict, 5),
    ] {
        let encoded = to_bytes(context, &class).expect("serialize attribution class");
        assert_eq!(AttributionClass::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
        let decoded: AttributionClass = encoded
            .deserialize()
            .expect("deserialize attribution class")
            .0;
        assert_eq!(decoded, class);
    }

    for (policy, discriminant) in [
        (InlineReplyPolicy::Allow, 0_u8),
        (InlineReplyPolicy::Deny, 2),
    ] {
        let encoded = to_bytes(context, &policy).expect("serialize inline reply policy");
        assert_eq!(InlineReplyPolicy::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
        let decoded: InlineReplyPolicy = encoded
            .deserialize()
            .expect("deserialize inline reply policy")
            .0;
        assert_eq!(decoded, policy);
    }
}

#[test]
fn attribution_wire_enums_reject_unknown_discriminants() {
    let context = Context::new_dbus(LE, 0);

    // Representation-aware deserialization must not invent policy for unknown wire values
    let unknown_class = to_bytes(context, &u8::MAX).expect("serialize unknown class byte");
    assert!(unknown_class.deserialize::<AttributionClass>().is_err());

    // The intentionally unused policy value must remain invalid on D-Bus
    let unknown_policy = to_bytes(context, &1_u8).expect("serialize unused policy byte");
    assert!(unknown_policy.deserialize::<InlineReplyPolicy>().is_err());
}

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
