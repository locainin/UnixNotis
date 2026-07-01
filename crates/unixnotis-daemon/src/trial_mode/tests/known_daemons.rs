use super::KNOWN_DAEMONS;

#[test]
fn known_daemons_include_quickshell_owner() {
    // Quickshell can own org.freedesktop.Notifications directly
    let quickshell = KNOWN_DAEMONS
        .iter()
        .find(|daemon| daemon.name == "quickshell")
        .expect("quickshell should be known");

    // The unit name lets auto restore prefer systemd when available
    assert_eq!(quickshell.unit, "quickshell.service");
}
