use super::Urgency;
use zbus::zvariant::OwnedValue;

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
