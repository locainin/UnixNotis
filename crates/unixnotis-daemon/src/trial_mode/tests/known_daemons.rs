use super::KNOWN_DAEMONS;

#[test]
fn known_daemons_include_quickshell_owner() {
    // Quickshell can own org.freedesktop.Notifications directly
    let quickshell = KNOWN_DAEMONS
        .iter()
        .find(|daemon| daemon.name == "quickshell")
        .expect("quickshell should be known");

    // The unit name lets auto restore prefer systemd when available
    assert_eq!(quickshell.systemd_unit, None);
}

#[test]
fn known_daemons_include_fnott_owner_and_real_service_unit() {
    let fnott = KNOWN_DAEMONS
        .iter()
        .find(|daemon| daemon.name == "fnott")
        .expect("fnott should be known");

    assert_eq!(fnott.systemd_unit, Some("fnott.service"));
}

#[test]
fn trial_and_installer_share_the_complete_daemon_catalog() {
    assert!(KNOWN_DAEMONS.len() >= 16);
    assert!(KNOWN_DAEMONS
        .iter()
        .any(|daemon| daemon.name == "lxqt-notificationd"));
    assert!(KNOWN_DAEMONS.iter().any(|daemon| daemon.name == "runst"));
}
