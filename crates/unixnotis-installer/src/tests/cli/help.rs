use super::{parse_args, test_support, usage, CliAction};

#[test]
fn help_short_circuits_tui_startup() {
    // Help should return a distinct action so caller code can print usage
    // and exit without initializing installer state
    let parsed = parse_args(test_support::args(&["--help"])).expect("valid args");

    assert!(matches!(parsed, CliAction::Help));
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
}
