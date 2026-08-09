use std::fs;
use std::os::unix::fs::symlink;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::super::{
    command_routing::use_fake_command_bin, CommandSpec, ServiceManagerAvailability,
    ServiceManagerAvailabilityOutput, ServiceManagerAvailabilityProbe,
};

fn success_is_available(
    output: ServiceManagerAvailabilityOutput<'_>,
) -> ServiceManagerAvailability {
    if output.status_success() {
        ServiceManagerAvailability::Available
    } else {
        ServiceManagerAvailability::Indeterminate
    }
}

fn successful_output_is_available(
    output: ServiceManagerAvailabilityOutput<'_>,
) -> ServiceManagerAvailability {
    if output.status_success() && output.did_exit() {
        ServiceManagerAvailability::Available
    } else {
        ServiceManagerAvailability::Indeterminate
    }
}

fn normal_exit_is_available(
    output: ServiceManagerAvailabilityOutput<'_>,
) -> ServiceManagerAvailability {
    if output.did_exit() {
        ServiceManagerAvailability::Available
    } else {
        ServiceManagerAvailability::Indeterminate
    }
}

struct TempDirGuard {
    path: std::path::PathBuf,
}

impl TempDirGuard {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "unixnotis-manager-availability-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create availability test directory");
        Self { path }
    }

    fn link_shell(&self, name: &str) {
        symlink("/bin/sh", self.path.join(name)).expect("link fake availability tool");
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn successful_transport_query_reports_manager_available() {
    let root = TempDirGuard::new("available");
    root.link_shell("managerctl");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceManagerAvailabilityProbe::new(
        CommandSpec::new("manager availability", "managerctl", ["-c", "exit 0"]),
        success_is_available,
    );

    assert_eq!(
        probe.evaluate().expect("availability probe should run"),
        ServiceManagerAvailability::Available
    );
}

#[test]
fn generic_nonzero_result_remains_indeterminate_until_a_backend_recognizes_it() {
    let root = TempDirGuard::new("indeterminate-failure");
    root.link_shell("managerctl");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceManagerAvailabilityProbe::new(
        CommandSpec::new("manager availability", "managerctl", ["-c", "exit 1"]),
        success_is_available,
    );

    assert_eq!(
        probe
            .evaluate()
            .expect("backend interpretation should return a stable state"),
        ServiceManagerAvailability::Indeterminate
    );
}

#[test]
fn signal_terminated_query_remains_indeterminate() {
    let root = TempDirGuard::new("signal-terminated");
    root.link_shell("managerctl");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceManagerAvailabilityProbe::new(
        CommandSpec::new(
            "signal-terminated manager availability",
            "managerctl",
            ["-c", "kill -TERM $$"],
        ),
        normal_exit_is_available,
    );

    assert_eq!(
        probe
            .evaluate()
            .expect("signal termination should remain a stable probe result"),
        ServiceManagerAvailability::Indeterminate
    );
}

#[test]
fn missing_manager_tool_reports_manager_unavailable() {
    let root = TempDirGuard::new("tool-unavailable");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceManagerAvailabilityProbe::new(
        CommandSpec::new(
            "missing manager availability",
            "missing-managerctl",
            ["status"],
        ),
        success_is_available,
    );

    assert_eq!(
        probe.evaluate().expect("missing manager is a stable state"),
        ServiceManagerAvailability::Unavailable
    );
}

#[test]
fn unsafe_manager_tool_object_remains_an_error() {
    let root = TempDirGuard::new("unsafe-tool");
    fs::create_dir(root.path.join("managerctl")).expect("create unsafe manager tool object");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceManagerAvailabilityProbe::new(
        CommandSpec::new("unsafe manager availability", "managerctl", ["status"]),
        success_is_available,
    );

    probe
        .evaluate()
        .expect_err("an unsafe manager tool must not look unavailable");
}

#[test]
fn availability_timeout_remains_an_error() {
    let root = TempDirGuard::new("timeout");
    root.link_shell("managerctl");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceManagerAvailabilityProbe::new(
        CommandSpec::new(
            "timed manager availability",
            "managerctl",
            ["-c", "sleep 30"],
        ),
        success_is_available,
    );

    let error = probe
        .evaluate_with_timeout(Duration::from_millis(25))
        .expect_err("a hung manager must not look unavailable");

    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
}

#[test]
fn moderate_availability_output_remains_within_the_capture_budget() {
    let root = TempDirGuard::new("moderate-output");
    root.link_shell("managerctl");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceManagerAvailabilityProbe::new(
        CommandSpec::new(
            "manager availability with moderate output",
            "managerctl",
            [
                "-c",
                "i=0; while [ \"$i\" -lt 2048 ]; do printf x; i=$((i + 1)); done",
            ],
        ),
        successful_output_is_available,
    );

    assert_eq!(
        probe.evaluate().expect("moderate output must stay bounded"),
        ServiceManagerAvailability::Available
    );
}

#[test]
fn oversized_stdout_is_rejected_without_requiring_oversized_stderr() {
    let root = TempDirGuard::new("oversized-stdout");
    root.link_shell("managerctl");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceManagerAvailabilityProbe::new(
        CommandSpec::new(
            "manager availability with oversized stdout",
            "managerctl",
            [
                "-c",
                "i=0; while [ \"$i\" -lt 17000 ]; do printf x; i=$((i + 1)); done",
            ],
        ),
        success_is_available,
    );

    let error = probe
        .evaluate()
        .expect_err("oversized stdout must not reach the backend parser");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn oversized_stderr_is_rejected_without_requiring_oversized_stdout() {
    let root = TempDirGuard::new("oversized-stderr");
    root.link_shell("managerctl");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceManagerAvailabilityProbe::new(
        CommandSpec::new(
            "manager availability with oversized stderr",
            "managerctl",
            [
                "-c",
                "i=0; while [ \"$i\" -lt 17000 ]; do printf x >&2; i=$((i + 1)); done",
            ],
        ),
        success_is_available,
    );

    let error = probe
        .evaluate()
        .expect_err("oversized stderr must not reach the backend parser");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}
