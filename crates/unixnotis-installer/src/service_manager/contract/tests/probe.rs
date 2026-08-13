use std::fs;
use std::os::unix::fs::symlink;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::command_routing::use_fake_command_bin;
use crate::service_manager::contract::{ServiceProbe, ServiceProbeState};
use crate::service_manager::CommandSpec;

impl ServiceProbe {
    pub(crate) const fn command(&self) -> &CommandSpec {
        &self.command
    }

    pub(crate) fn parser_state(&self, status_success: bool, stdout: &str) -> ServiceProbeState {
        self.parser_state_with_result(if status_success { Some(0) } else { Some(1) }, stdout, "")
    }

    pub(crate) fn parser_state_with_result(
        &self,
        status_code: Option<i32>,
        stdout: &str,
        stderr: &str,
    ) -> ServiceProbeState {
        (self.interpret)(super::super::ServiceProbeOutput {
            status_success: status_code == Some(0),
            status_code,
            stdout,
            stderr,
        })
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
            "unixnotis-service-probe-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn link_shell(&self, name: &str) {
        // Stable shell links avoid transient executable-busy failures during mutation runs
        symlink("/bin/sh", self.path.join(name)).expect("link fake probe tool");
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn explicit_probe_state_is_returned_after_successful_command() {
    let root = TempDirGuard::new("success");
    root.link_shell("probe-tool");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceProbe::new(
        CommandSpec::new("probe", "probe-tool", ["-c", "printf 'true\\n'; exit 0"]),
        |output| {
            if output.status_success() && output.stdout().trim() == "true" {
                ServiceProbeState::Active
            } else {
                ServiceProbeState::Indeterminate
            }
        },
    );

    let state = probe.evaluate_state().expect("probe should run");

    assert_eq!(state, ServiceProbeState::Active);
}

#[test]
fn failed_command_is_indeterminate_even_with_active_looking_output() {
    let root = TempDirGuard::new("failure");
    root.link_shell("probe-tool");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceProbe::new(
        CommandSpec::new("probe", "probe-tool", ["-c", "printf 'true\\n'; exit 1"]),
        |output| {
            if output.status_success() && output.stdout().trim() == "true" {
                ServiceProbeState::Active
            } else {
                ServiceProbeState::Indeterminate
            }
        },
    );

    let state = probe.evaluate_state().expect("probe should run");

    assert_eq!(state, ServiceProbeState::Indeterminate);
}

#[test]
fn missing_probe_program_is_classified_as_unavailable() {
    let root = TempDirGuard::new("unavailable");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceProbe::new(
        CommandSpec::new("missing manager probe", "missing-managerctl", ["status"]),
        |_output| ServiceProbeState::Indeterminate,
    );

    let state = probe
        .evaluate_state()
        .expect("missing alternate manager should have a stable state");

    assert_eq!(state, ServiceProbeState::Unavailable);
}

#[test]
fn unsafe_probe_program_is_an_error_instead_of_unavailable() {
    let root = TempDirGuard::new("unsafe-program");
    fs::create_dir(root.path.join("probe-tool")).expect("create unsafe probe tool object");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceProbe::new(
        CommandSpec::new("unsafe manager probe", "probe-tool", ["status"]),
        |_output| ServiceProbeState::Inactive,
    );

    probe
        .evaluate_state()
        .expect_err("an unsafe program must not be classified as unavailable");
}

#[test]
fn oversized_stdout_is_rejected_even_when_stderr_is_empty() {
    let root = TempDirGuard::new("oversized-stdout");
    root.link_shell("probe-tool");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceProbe::new(
        CommandSpec::new(
            "oversized stdout probe",
            "probe-tool",
            ["-c", "head -c 32768 /dev/zero"],
        ),
        |_output| ServiceProbeState::Inactive,
    );

    let error = probe
        .evaluate_state()
        .expect_err("oversized stdout must fail independently of stderr");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn oversized_stderr_is_rejected_even_when_stdout_is_empty() {
    let root = TempDirGuard::new("oversized-stderr");
    root.link_shell("probe-tool");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceProbe::new(
        CommandSpec::new(
            "oversized stderr probe",
            "probe-tool",
            ["-c", "head -c 32768 /dev/zero >&2"],
        ),
        |_output| ServiceProbeState::Inactive,
    );

    let error = probe
        .evaluate_state()
        .expect_err("oversized stderr must fail independently of stdout");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn malformed_stderr_is_indeterminate_before_backend_interpretation() {
    let root = TempDirGuard::new("malformed-stderr");
    root.link_shell("probe-tool");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceProbe::new(
        CommandSpec::new(
            "malformed stderr probe",
            "probe-tool",
            ["-c", "printf '\\377' >&2"],
        ),
        |_output| ServiceProbeState::Inactive,
    );

    let state = probe
        .evaluate_state()
        .expect("malformed manager output has a stable fail-closed state");

    assert_eq!(state, ServiceProbeState::Indeterminate);
}

#[test]
fn probe_timeout_is_never_reported_as_inactive() {
    let root = TempDirGuard::new("timeout");
    root.link_shell("probe-tool");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceProbe::new(
        CommandSpec::new("timed probe", "probe-tool", ["-c", "sleep 30"]),
        |_output| ServiceProbeState::Inactive,
    );

    let error = probe
        .evaluate_state_with_timeout(std::time::Duration::from_millis(25))
        .expect_err("a timed-out probe must remain indeterminate");

    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
}
