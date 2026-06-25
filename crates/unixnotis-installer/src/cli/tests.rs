use super::{parse_args, CliAction};
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
fn service_manager_flag_rejects_unknown_value() {
    // Unsupported backends must fail loudly instead of silently falling back
    // to systemd or another default
    let err = parse_args(args(&["--service-manager", "launchd"])).expect_err("invalid args");

    // The precise list of supported managers lives in `ServiceManagerChoice`,
    // but this test makes sure the error path is connected to CLI parsing
    assert!(err.to_string().contains("unsupported service manager"));
}
