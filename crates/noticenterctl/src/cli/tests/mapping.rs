use unixnotis_core::{PanelDebugLevel, INHIBIT_SCOPE_ALL, INHIBIT_SCOPE_POPUPS};

use super::super::{DebugLevelArg, InhibitScopeArg};

#[test]
fn debug_level_arg_into_panel_level() {
    // Validates CLI debug levels map to the matching control plane enum
    let table = [
        (DebugLevelArg::Critical, PanelDebugLevel::Critical),
        (DebugLevelArg::Warn, PanelDebugLevel::Warn),
        (DebugLevelArg::Info, PanelDebugLevel::Info),
        (DebugLevelArg::Verbose, PanelDebugLevel::Verbose),
    ];
    for (arg, expected) in table {
        let mapped: PanelDebugLevel = arg.into();
        assert_eq!(mapped, expected);
    }
}

#[test]
fn inhibit_scope_arg_maps_to_control_bitmasks() {
    assert_eq!(InhibitScopeArg::All.as_scope(), INHIBIT_SCOPE_ALL);
    assert_eq!(InhibitScopeArg::Popups.as_scope(), INHIBIT_SCOPE_POPUPS);
}
