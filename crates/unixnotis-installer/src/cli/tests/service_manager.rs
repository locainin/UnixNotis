use super::{parse_args, test_support};
use crate::paths::ServiceManagerChoice;

#[test]
fn service_manager_flag_accepts_separate_value() {
    // Covers:
    //
    //   unixnotis-installer --service-manager runit
    let parsed =
        parse_args(test_support::args(&["--service-manager", "runit"])).expect("valid args");
    let args = test_support::run_args(parsed);

    // The flag should not short-circuit startup; it should produce normal
    // run arguments with the requested backend override
    assert_eq!(args.service_manager, Some(ServiceManagerChoice::Runit));
}

#[test]
fn service_manager_flag_accepts_equals_value() {
    // Covers:
    //
    //   unixnotis-installer --service-manager=dinit
    let parsed = parse_args(test_support::args(&["--service-manager=dinit"])).expect("valid args");
    let args = test_support::run_args(parsed);

    // Equals-form parsing should produce the same normal run action as
    // split-form parsing
    assert_eq!(args.service_manager, Some(ServiceManagerChoice::Dinit));
}

#[test]
fn service_manager_flag_accepts_s6_value() {
    // Ensure the CLI accepts the experimental s6 backend selector
    let parsed = parse_args(test_support::args(&["--service-manager", "s6"])).expect("valid args");
    let args = test_support::run_args(parsed);

    // The selected backend is stored as data; path discovery and backend
    // construction happen elsewhere
    assert_eq!(args.service_manager, Some(ServiceManagerChoice::S6));
}

#[test]
fn service_manager_split_form_requires_value() {
    let err = parse_args(test_support::args(&["--service-manager"])).expect_err("missing value");

    // Missing values should point at the flag that consumed the next argument
    assert_eq!(err.to_string(), "--service-manager requires a value");
}

#[test]
fn service_manager_flag_rejects_unknown_value() {
    // Unsupported backends must fail loudly instead of silently falling back
    // to systemd or another default
    let err = parse_args(test_support::args(&["--service-manager", "launchd"]))
        .expect_err("invalid args");

    // The precise list of supported managers lives in `ServiceManagerChoice`,
    // but this test makes sure the error path is connected to CLI parsing
    assert!(err.to_string().contains("unsupported service manager"));
}

#[test]
fn service_manager_equals_form_rejects_empty_value() {
    let err =
        parse_args(test_support::args(&["--service-manager="])).expect_err("empty manager value");

    // Empty equals-form values should stay attached to service-manager parsing
    // so users get the backend-name error instead of a generic argument error
    assert!(err.to_string().contains("unsupported service manager ''"));
}
