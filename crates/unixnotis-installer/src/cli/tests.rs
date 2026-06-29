use super::{parse_args, usage, CliAction};
use crate::paths::ServiceManagerChoice;

/// Convenience helper for building test argument vectors
///
/// The parser accepts `OsString`, but most tests only need ordinary UTF-8
/// string literals
fn args(values: &[&str]) -> Vec<std::ffi::OsString> {
    values.iter().map(std::ffi::OsString::from).collect()
}

#[test]
fn service_manager_flag_accepts_separate_value() {
    // Covers:
    //
    //   unixnotis-installer --service-manager runit
    let parsed = parse_args(args(&["--service-manager", "runit"])).expect("valid args");

    // The flag should not short-circuit startup; it should produce normal
    // run arguments with the requested backend override
    let CliAction::Run(args) = parsed else {
        panic!("expected run action");
    };

    assert_eq!(args.service_manager, Some(ServiceManagerChoice::Runit));
}

#[test]
fn service_manager_flag_accepts_equals_value() {
    // Covers:
    //
    //   unixnotis-installer --service-manager=dinit
    let parsed = parse_args(args(&["--service-manager=dinit"])).expect("valid args");

    // Equals-form parsing should produce the same kind of normal run action
    // as split-form parsing
    let CliAction::Run(args) = parsed else {
        panic!("expected run action");
    };

    assert_eq!(args.service_manager, Some(ServiceManagerChoice::Dinit));
}

#[test]
fn service_manager_flag_accepts_s6_value() {
    // Ensure the CLI already accepts the planned s6 backend selector
    let parsed = parse_args(args(&["--service-manager", "s6"])).expect("valid args");

    // The selected backend is stored as data; actual path discovery and
    // backend construction happen elsewhere
    let CliAction::Run(args) = parsed else {
        panic!("expected run action");
    };

    assert_eq!(args.service_manager, Some(ServiceManagerChoice::S6));
}

#[test]
fn help_short_circuits_tui_startup() {
    // Help should return a distinct action so caller code can print usage
    // and exit without initializing installer state
    let parsed = parse_args(args(&["--help"])).expect("valid args");

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

#[test]
fn service_manager_flag_rejects_unknown_value() {
    // Unsupported backends must fail loudly instead of silently falling back
    // to systemd or another default
    let err = parse_args(args(&["--service-manager", "launchd"])).expect_err("invalid args");

    // The precise list of supported managers lives in `ServiceManagerChoice`,
    // but this test makes sure the error path is connected to CLI parsing
    assert!(err.to_string().contains("unsupported service manager"));
}

#[test]
fn service_manager_equals_form_rejects_empty_value() {
    let err = parse_args(args(&["--service-manager="])).expect_err("empty manager value");

    // Empty equals-form values should stay attached to service-manager parsing
    // so users get the backend-name error instead of a generic argument error
    assert!(err.to_string().contains("unsupported service manager ''"));
}

#[test]
fn unknown_equals_argument_is_not_treated_as_service_manager() {
    let err = parse_args(args(&["--other=systemd"])).expect_err("unknown argument");

    // This guards the `--service-manager=` prefix check. A loose match would
    // incorrectly accept unrelated `--name=systemd` style flags
    assert!(err
        .to_string()
        .contains("unsupported installer argument '--other=systemd'"));
}

#[test]
fn unsupported_argument_reports_original_text() {
    let err = parse_args(args(&["--bogus"])).expect_err("unknown argument");

    // Keep unsupported argument diagnostics specific enough for TUI/CLI users
    // to find the typo without guessing which parser branch handled it
    assert_eq!(err.to_string(), "unsupported installer argument '--bogus'");
}
