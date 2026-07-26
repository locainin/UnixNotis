use super::Urgency;
use zbus::zvariant::{serialized::Context, to_bytes, OwnedValue, Type, LE};

#[test]
fn urgency_wire_values_use_their_declared_one_byte_signature() {
    let context = Context::new_dbus(LE, 0);

    for (urgency, discriminant) in [
        (Urgency::Low, 0_u8),
        (Urgency::Normal, 1),
        (Urgency::Critical, 2),
    ] {
        let encoded = to_bytes(context, &urgency).expect("serialize urgency");
        assert_eq!(Urgency::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
        let decoded: Urgency = encoded.deserialize().expect("deserialize urgency").0;
        assert_eq!(decoded, urgency);
    }
}

#[test]
fn urgency_wire_values_reject_unknown_discriminants() {
    let context = Context::new_dbus(LE, 0);
    let encoded = to_bytes(context, &u8::MAX).expect("serialize unknown urgency byte");

    assert!(encoded.deserialize::<Urgency>().is_err());
}

#[test]
fn urgency_hint_maps_known_values_to_protocol_urgency() {
    // Clients commonly send either byte or wider integer urgency hints
    assert_eq!(
        Urgency::from_hint(Some(&OwnedValue::from(0_u8))),
        Urgency::Low
    );
    assert_eq!(
        Urgency::from_hint(Some(&OwnedValue::from(1_u8))),
        Urgency::Normal
    );
    assert_eq!(
        Urgency::from_hint(Some(&OwnedValue::from(2_u32))),
        Urgency::Critical
    );
}

#[test]
fn urgency_hint_defaults_unknown_or_missing_values_to_normal() {
    assert_eq!(Urgency::from_hint(None), Urgency::Normal);
    assert_eq!(
        Urgency::from_hint(Some(&OwnedValue::from(9_u32))),
        Urgency::Normal
    );
    assert_eq!(
        Urgency::from_hint(Some(&OwnedValue::from(true))),
        Urgency::Normal
    );
}

#[test]
fn urgency_as_u8_matches_freedesktop_values() {
    assert_eq!(Urgency::Low.as_u8(), 0);
    assert_eq!(Urgency::Normal.as_u8(), 1);
    assert_eq!(Urgency::Critical.as_u8(), 2);
}
