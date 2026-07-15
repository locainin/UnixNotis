use std::ffi::OsString;

use super::{parse_args, test_support};

#[test]
fn unknown_equals_argument_is_not_treated_as_service_manager() {
    let err = parse_args(test_support::args(&["--other=systemd"])).expect_err("unknown argument");

    // This guards the `--service-manager=` prefix check. A loose match would
    // incorrectly accept unrelated `--name=systemd` style flags
    assert!(err
        .to_string()
        .contains("unsupported installer argument '--other=systemd'"));
}

#[test]
fn unsupported_argument_reports_original_text() {
    let err = parse_args(test_support::args(&["--bogus"])).expect_err("unknown argument");

    // Keep unsupported argument diagnostics specific enough for TUI/CLI users
    // to find the typo without guessing which parser branch handled it
    assert_eq!(err.to_string(), "unsupported installer argument '--bogus'");
}

#[cfg(unix)]
#[test]
fn invalid_utf8_argument_is_rejected_before_flag_matching() {
    use std::os::unix::ffi::OsStringExt;

    let invalid = OsString::from_vec(vec![0xff, b'-', b'-']);
    let err = parse_args([invalid]).expect_err("invalid UTF-8 argument");

    // Raw Unix argv can contain non-UTF-8 bytes. Rejecting it at the parser
    // boundary keeps later flag matching simple and deterministic
    assert_eq!(err.to_string(), "installer arguments must be valid UTF-8");
}
