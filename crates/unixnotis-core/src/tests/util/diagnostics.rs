use super::*;

#[test]
fn diagnostic_mode_parses_expected_values() {
    assert!(diagnostic_mode_from(Some("1")));
    assert!(diagnostic_mode_from(Some("true")));
    assert!(diagnostic_mode_from(Some("YES")));
    assert!(diagnostic_mode_from(Some("on")));
    assert!(!diagnostic_mode_from(Some("0")));
    assert!(!diagnostic_mode_from(Some("false")));
    assert!(!diagnostic_mode_from(None));
}

#[test]
fn diagnostic_mode_reads_environment_wrapper() {
    let _guard = crate::test_support::test_env_lock();
    let previous = crate::test_support::set_env("UNIXNOTIS_DIAGNOSTIC", Some("yes"));
    assert!(diagnostic_mode());

    std::env::set_var("UNIXNOTIS_DIAGNOSTIC", "off");
    assert!(!diagnostic_mode());

    crate::test_support::restore_env("UNIXNOTIS_DIAGNOSTIC", previous);
}

#[test]
fn log_limit_respects_mode() {
    assert_eq!(log_limit_for(false), DEFAULT_LOG_LIMIT);
    assert_eq!(log_limit_for(true), DIAGNOSTIC_LOG_LIMIT);
}

#[test]
fn log_limit_and_snippet_use_diagnostic_environment() {
    let _guard = crate::test_support::test_env_lock();
    let previous = crate::test_support::set_env("UNIXNOTIS_DIAGNOSTIC", Some("true"));
    assert_eq!(log_limit(), DIAGNOSTIC_LOG_LIMIT);

    std::env::set_var("UNIXNOTIS_DIAGNOSTIC", "false");
    assert_eq!(log_limit(), DEFAULT_LOG_LIMIT);

    let noisy = "value\nwith\rcontrols";
    assert_eq!(log_snippet(noisy), "value with controls");

    crate::test_support::restore_env("UNIXNOTIS_DIAGNOSTIC", previous);
}

#[test]
fn diagnostic_limits_are_distinct_and_ordered() {
    // Diagnostic mode intentionally keeps longer snippets for manual troubleshooting
    assert_eq!(default_log_limit(), DEFAULT_LOG_LIMIT);
    assert_eq!(diagnostic_log_limit(), DIAGNOSTIC_LOG_LIMIT);
    assert!(diagnostic_log_limit() > default_log_limit());
}
