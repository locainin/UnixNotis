use super::{parse_args, test_support, usage, version, CliAction};

#[test]
fn help_short_circuits_tui_startup() {
    // Help should return a distinct action so caller code can print usage
    // and exit without initializing installer state
    let parsed = parse_args(test_support::args(&["--help"])).expect("valid args");

    assert!(matches!(parsed, CliAction::Help));
}

#[test]
fn short_help_short_circuits_tui_startup() {
    let parsed = parse_args(test_support::args(&["-h"])).expect("valid args");

    assert!(matches!(parsed, CliAction::Help));
}

#[test]
fn version_short_circuits_tui_startup() {
    let parsed = parse_args(test_support::args(&["--version"])).expect("valid args");

    assert!(matches!(parsed, CliAction::Version));
    assert_eq!(version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn usage_mentions_every_supported_service_manager() {
    let text = usage();

    // Keep help output tied to the actual public backend names. This catches
    // empty or stale usage text before it reaches release builds
    for expected in ["systemd", "dinit", "runit", "s6"] {
        assert!(
            text.contains(expected),
            "usage text should mention {expected}"
        );
    }
    assert!(text.contains("-h|--help"));
    assert!(text.contains("-V|--version"));
}
