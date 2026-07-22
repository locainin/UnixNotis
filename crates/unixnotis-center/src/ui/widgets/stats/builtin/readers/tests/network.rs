//! Network reader selection tests

use super::{pick_default_iface_from, IfaceCandidate};

#[test]
fn default_interface_prefers_an_active_physical_device() {
    let candidates = vec![
        IfaceCandidate {
            name: "veth0".to_string(),
            operstate: "up".to_string(),
        },
        IfaceCandidate {
            name: "wlan0".to_string(),
            operstate: "up".to_string(),
        },
    ];

    assert_eq!(
        pick_default_iface_from(&candidates),
        Some("wlan0".to_string())
    );
}

#[test]
fn default_interface_prefers_physical_devices_when_all_are_down() {
    let candidates = vec![
        IfaceCandidate {
            name: "eth0".to_string(),
            operstate: "down".to_string(),
        },
        IfaceCandidate {
            name: "docker0".to_string(),
            operstate: "up".to_string(),
        },
    ];

    assert_eq!(
        pick_default_iface_from(&candidates),
        Some("eth0".to_string())
    );
}

#[test]
fn default_interface_uses_name_as_a_deterministic_tiebreaker() {
    let candidates = vec![
        IfaceCandidate {
            name: "eth1".to_string(),
            operstate: "down".to_string(),
        },
        IfaceCandidate {
            name: "eth0".to_string(),
            operstate: "down".to_string(),
        },
    ];

    assert_eq!(
        pick_default_iface_from(&candidates),
        Some("eth0".to_string())
    );
}
