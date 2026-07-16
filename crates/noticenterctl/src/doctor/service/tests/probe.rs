use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use unixnotis_core::service_manager::{ServiceManagerKind, ServiceManagerPaths};

use super::super::probe::{
    active_candidate, sanitize_output, status_check, status_command_for_unit,
    status_command_with_env, status_is_active,
};
use crate::doctor::report::DoctorSeverity;

struct FakeToolDirectory {
    path: PathBuf,
}

impl FakeToolDirectory {
    fn new() -> Self {
        static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
        let serial = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "unixnotis-doctor-service-tools-{}-{serial}",
            std::process::id(),
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create fake tool directory");
        Self { path }
    }

    fn write(&self, name: &str, output: &str) {
        self.write_with_exit(name, output, 0);
    }

    fn write_with_exit(&self, name: &str, output: &str, exit_code: u8) {
        let path = self.path.join(name);
        let script = format!("#!/bin/sh\nprintf '%s' '{output}'\nexit {exit_code}\n");
        std::fs::write(&path, script).expect("write fake service tool");
        let mut permissions = std::fs::metadata(&path)
            .expect("read fake service tool metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("make fake service tool executable");
    }
}

impl Drop for FakeToolDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn paths(kind: ServiceManagerKind) -> ServiceManagerPaths {
    ServiceManagerPaths {
        kind,
        artifact_root: PathBuf::from("/tmp/unixnotis-service-artifacts"),
        live_root: (kind == ServiceManagerKind::S6)
            .then(|| PathBuf::from("/tmp/unixnotis-service-live")),
    }
}

#[test]
fn systemd_requires_a_loaded_running_process_with_a_real_pid() {
    assert!(status_is_active(
        ServiceManagerKind::Systemd,
        true,
        "LoadState=loaded\nActiveState=active\nSubState=running\nExecMainPID=42"
    ));
    for output in [
        "LoadState=loaded\nActiveState=active\nSubState=exited\nExecMainPID=42",
        "LoadState=loaded\nActiveState=active\nSubState=running\nExecMainPID=0",
        "LoadState=not-found\nActiveState=active\nSubState=running\nExecMainPID=42",
    ] {
        assert!(!status_is_active(ServiceManagerKind::Systemd, true, output));
    }
}

#[test]
fn other_status_parsers_require_their_documented_active_shape() {
    assert!(status_is_active(ServiceManagerKind::Dinit, true, ""));
    assert!(status_is_active(
        ServiceManagerKind::Runit,
        true,
        "run: service"
    ));
    assert!(!status_is_active(
        ServiceManagerKind::Runit,
        true,
        "down: service"
    ));
    assert!(status_is_active(ServiceManagerKind::S6, true, "true"));
    assert!(!status_is_active(ServiceManagerKind::S6, true, "false"));
}

#[test]
fn systemd_status_places_options_before_the_protected_unit_operand() {
    let (_, args) = status_command_for_unit(
        ServiceManagerKind::Systemd,
        &paths(ServiceManagerKind::Systemd),
        "custom.service",
    );

    assert_eq!(args[args.len() - 2], "--");
    assert_eq!(args.last().map(String::as_str), Some("custom.service"));
    assert!(args[..args.len() - 2]
        .iter()
        .all(|argument| argument != "custom.service"));
}

#[test]
fn non_systemd_status_does_not_validate_the_systemd_unit_override() {
    let result = status_command_with_env(
        ServiceManagerKind::Dinit,
        &paths(ServiceManagerKind::Dinit),
        |_| Ok("--invalid.service".to_string()),
    );

    assert!(result.is_ok());
}

#[test]
fn status_output_is_sanitized_redacted_and_bounded() {
    let home = std::env::var("HOME").expect("HOME");
    let raw = format!("active\u{1b}[31m {home}/private\n{}", "x".repeat(8 * 1024));

    let sanitized = sanitize_output(raw.as_bytes());

    assert!(!sanitized.contains('\u{1b}'));
    assert!(!sanitized.contains(&home));
    assert!(sanitized.contains("$HOME/private"));
    assert!(sanitized.len() <= 4 * 1024);
}

#[test]
fn status_output_limit_preserves_multiple_safe_lines_below_the_byte_budget() {
    let raw = ["a".repeat(900), "b".repeat(900), "c".repeat(900)].join("\n");

    let sanitized = sanitize_output(raw.as_bytes());

    assert_eq!(sanitized, raw);
    assert!(sanitized.len() > 1028);
    assert!(sanitized.len() < 4 * 1024);
}

#[tokio::test]
async fn non_systemd_status_checks_execute_their_native_read_only_probe() {
    let tools = FakeToolDirectory::new();
    tools.write("dinitctl", "");
    tools.write("sv", "run: unixnotis-daemon: (pid 42) 10s");
    tools.write("s6-svstat", "true");
    let _tools = crate::system_tools::use_fake_tool_bin(&tools.path);

    for kind in [
        ServiceManagerKind::Dinit,
        ServiceManagerKind::Runit,
        ServiceManagerKind::S6,
    ] {
        let check = status_check(kind, &paths(kind)).await;

        assert_eq!(check.severity, DoctorSeverity::Pass);
        assert_eq!(
            check
                .data
                .get("manager")
                .and_then(serde_json::Value::as_str),
            Some(kind.label())
        );
        assert_eq!(
            check
                .data
                .get("active")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert!(!check.data.contains_key("load_state"));
    }
}

#[tokio::test]
async fn active_candidate_distinguishes_active_inactive_and_unavailable_probes() {
    let tools = FakeToolDirectory::new();
    tools.write("dinitctl", "");
    let _tools = crate::system_tools::use_fake_tool_bin(&tools.path);
    let dinit_paths = paths(ServiceManagerKind::Dinit);

    assert_eq!(
        active_candidate(ServiceManagerKind::Dinit, &dinit_paths).await,
        (true, None)
    );

    tools.write_with_exit("dinitctl", "", 1);
    assert_eq!(
        active_candidate(ServiceManagerKind::Dinit, &dinit_paths).await,
        (false, None)
    );

    let (active, error) =
        active_candidate(ServiceManagerKind::S6, &paths(ServiceManagerKind::S6)).await;
    assert!(!active);
    assert!(error
        .as_deref()
        .is_some_and(|detail| detail.contains("s6-svstat")));
}
