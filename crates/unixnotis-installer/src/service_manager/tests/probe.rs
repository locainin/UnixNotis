use crate::service_manager::{CommandSpec, ServiceProbe};

#[test]
fn stdout_probe_uses_parser_only_after_successful_command() {
    let probe = ServiceProbe::stdout(
        CommandSpec::new("probe", "/bin/sh", ["-c", "printf 'true\\n'; exit 0"]),
        |stdout| stdout.trim() == "true",
    );

    let active = probe.evaluate().expect("probe should run");

    // Successful stdout probes are allowed to derive active state from command output
    assert!(active);
}

#[test]
fn stdout_probe_treats_failed_command_as_inactive_even_with_matching_output() {
    let probe = ServiceProbe::stdout(
        CommandSpec::new("probe", "/bin/sh", ["-c", "printf 'true\\n'; exit 1"]),
        |stdout| stdout.trim() == "true",
    );

    let active = probe.evaluate().expect("probe should run");

    // Command failure means the manager did not provide trustworthy status output
    assert!(!active);
}
