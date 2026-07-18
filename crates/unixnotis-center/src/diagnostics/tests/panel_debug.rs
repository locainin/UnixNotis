use unixnotis_core::PanelDebugLevel;

use super::super::panel_debug::level_from_raw;

#[test]
fn raw_debug_levels_map_to_supported_values() {
    assert_eq!(level_from_raw(0), PanelDebugLevel::Off);
    assert_eq!(level_from_raw(1), PanelDebugLevel::Critical);
    assert_eq!(level_from_raw(2), PanelDebugLevel::Warn);
    assert_eq!(level_from_raw(3), PanelDebugLevel::Info);
    assert_eq!(level_from_raw(4), PanelDebugLevel::Verbose);
}

#[test]
fn unknown_debug_levels_fail_closed() {
    assert_eq!(level_from_raw(u8::MAX), PanelDebugLevel::Off);
}
