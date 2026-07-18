use super::{MediaRefreshOrigin, MediaSignal};

#[test]
fn property_signal_preserves_player_and_refresh_origin() {
    let signal = MediaSignal::PropertiesChanged {
        bus_name: "org.mpris.MediaPlayer2.test".to_string(),
        origin: MediaRefreshOrigin::Fallback,
    };

    let MediaSignal::PropertiesChanged { bus_name, origin } = signal;
    assert_eq!(bus_name, "org.mpris.MediaPlayer2.test");
    assert_eq!(origin, MediaRefreshOrigin::Fallback);
}

#[test]
fn native_and_synthetic_refresh_origins_remain_distinct() {
    assert_ne!(MediaRefreshOrigin::Bus, MediaRefreshOrigin::Fallback);
}
