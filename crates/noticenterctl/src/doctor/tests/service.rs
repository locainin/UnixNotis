use super::*;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;

fn paths(kind: ServiceManagerKind) -> ServiceManagerPaths {
    ServiceManagerPaths {
        kind,
        artifact_root: PathBuf::from("/tmp/unixnotis-service-artifacts"),
        live_root: (kind == ServiceManagerKind::S6)
            .then(|| PathBuf::from("/tmp/unixnotis-service-live")),
    }
}

#[test]
fn every_service_status_parser_requires_its_real_active_shape() {
    assert!(status_is_active(
        ServiceManagerKind::Systemd,
        true,
        "LoadState=loaded\nActiveState=active\nSubState=running"
    ));
    assert!(!status_is_active(
        ServiceManagerKind::Systemd,
        true,
        "ActiveState=inactive"
    ));
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
fn failed_process_status_never_counts_as_active() {
    for manager in ServiceManagerKind::all() {
        assert!(!status_is_active(
            manager,
            false,
            "ActiveState=active\nrun:\ntrue"
        ));
    }
}

#[test]
fn status_output_is_sanitized_and_bounded() {
    let home = std::env::var("HOME").expect("HOME");
    let raw = format!(
        "active\u{1b}[31m {home}/private\n{}",
        "x".repeat(SERVICE_OUTPUT_LIMIT * 2)
    );
    let sanitized = sanitize_output(raw.as_bytes());

    assert!(!sanitized.contains('\u{1b}'));
    assert_eq!(SERVICE_OUTPUT_LIMIT, 4096);
    assert!(sanitized.len() <= 4096);
    assert!(sanitized.contains('\n'));
    assert!(!sanitized.contains(&home));
    assert!(sanitized.contains("$HOME/private"));
}

#[test]
fn systemd_status_uses_the_shared_unit_override() {
    let paths = resolve_service_manager_paths(ServiceManagerKind::Systemd)
        .expect("resolve systemd service paths");
    let (_, args) = status_command_for_unit(
        ServiceManagerKind::Systemd,
        &paths,
        "custom-unixnotis.service",
    );

    assert!(args
        .iter()
        .any(|argument| argument == "custom-unixnotis.service"));
}

#[test]
fn environment_selection_ignores_empty_values_and_rejects_unknown_values() {
    assert_eq!(manager_from_environment(None).expect("missing value"), None);
    assert_eq!(
        manager_from_environment(Some(OsString::new())).expect("empty value"),
        None
    );
    assert_eq!(
        manager_from_environment(Some(OsString::from("dinit"))).expect("known value"),
        Some(SelectedServiceManager::Managed(ServiceManagerKind::Dinit))
    );
    let error = manager_from_environment(Some(OsString::from("unsupported")))
        .expect_err("unknown manager must fail");
    assert_eq!(error.severity, DoctorSeverity::Error);
}

#[test]
fn candidate_presence_accepts_an_artifact_or_an_active_probe() {
    assert!(!candidate_is_present(false, false));
    assert!(candidate_is_present(true, false));
    assert!(candidate_is_present(false, true));
    assert!(candidate_is_present(true, true));
}

#[test]
fn detected_manager_selection_handles_every_empty_and_ambiguous_case() {
    assert_eq!(
        select_detected_manager(&[ServiceManagerKind::Runit], &[], false)
            .expect("single candidate"),
        SelectedServiceManager::Managed(ServiceManagerKind::Runit)
    );
    assert_eq!(
        select_detected_manager(&[], &[], true).expect("reachable manual launch"),
        SelectedServiceManager::Manual
    );
    assert_eq!(
        select_detected_manager(&[], &[], false).expect("unknown launch"),
        SelectedServiceManager::Unknown
    );
    let path_error = select_detected_manager(&[], &["dinit: missing HOME".to_string()], false)
        .expect_err("incomplete path inspection must be reported");
    assert!(path_error
        .details
        .as_deref()
        .is_some_and(|details| details.contains("missing HOME")));
    let ambiguous = select_detected_manager(
        &[ServiceManagerKind::Systemd, ServiceManagerKind::Dinit],
        &[],
        false,
    )
    .expect_err("multiple candidates must be reported");
    assert!(ambiguous
        .details
        .as_deref()
        .is_some_and(|details| details == "systemd, dinit"));
}

#[test]
fn primary_artifacts_follow_each_installer_layout() {
    assert_eq!(
        primary_artifact(&paths(ServiceManagerKind::Systemd)),
        PathBuf::from("/tmp/unixnotis-service-artifacts/unixnotis-daemon.service")
    );
    assert_eq!(
        primary_artifact(&paths(ServiceManagerKind::Dinit)),
        PathBuf::from("/tmp/unixnotis-service-artifacts/unixnotis-daemon")
    );
    assert_eq!(
        primary_artifact(&paths(ServiceManagerKind::Runit)),
        PathBuf::from("/tmp/unixnotis-service-artifacts/unixnotis-daemon/run")
    );
    assert_eq!(
        primary_artifact(&paths(ServiceManagerKind::S6)),
        PathBuf::from("/tmp/unixnotis-service-artifacts/sv/unixnotis-daemon/run")
    );
}

#[test]
fn status_commands_match_each_installed_backend() {
    let systemd_paths = paths(ServiceManagerKind::Systemd);
    let systemd = status_command_for_unit(
        ServiceManagerKind::Systemd,
        &systemd_paths,
        "custom.service",
    );
    assert_eq!(systemd.0, "systemctl");
    assert_eq!(systemd.1[0..3], ["--user", "show", "custom.service"]);
    assert!(systemd.1.iter().any(|argument| argument == "--no-pager"));

    let dinit = status_command_for_unit(
        ServiceManagerKind::Dinit,
        &paths(ServiceManagerKind::Dinit),
        "ignored",
    );
    assert_eq!(
        dinit,
        (
            "dinitctl",
            vec![
                "--user".to_string(),
                "--quiet".to_string(),
                "is-started".to_string(),
                SERVICE_NAME.to_string()
            ]
        )
    );

    let runit = status_command_for_unit(
        ServiceManagerKind::Runit,
        &paths(ServiceManagerKind::Runit),
        "ignored",
    );
    assert_eq!(runit.0, "sv");
    assert_eq!(
        runit.1,
        vec![
            "status",
            "/tmp/unixnotis-service-artifacts/unixnotis-daemon"
        ]
    );

    let s6 = status_command_for_unit(
        ServiceManagerKind::S6,
        &paths(ServiceManagerKind::S6),
        "ignored",
    );
    assert_eq!(s6.0, "s6-svstat");
    assert_eq!(
        s6.1,
        vec![
            "-o",
            "up",
            "/tmp/unixnotis-service-live/servicedirs/unixnotis-daemon"
        ]
    );
}

#[test]
fn status_command_wrapper_preserves_the_resolved_command() {
    let paths = paths(ServiceManagerKind::Dinit);

    assert_eq!(
        status_command(ServiceManagerKind::Dinit, &paths),
        status_command_for_unit(ServiceManagerKind::Dinit, &paths, "ignored")
    );
}

#[tokio::test]
async fn active_probe_and_status_check_follow_real_command_output() {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-doctor-service-probe-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create fake tool directory");
    let systemctl = root.join("systemctl");
    std::fs::write(
        &systemctl,
        "#!/bin/sh\nprintf 'LoadState=loaded\\nActiveState=active\\nSubState=running\\n'\n",
    )
    .expect("write active systemctl probe");
    std::fs::set_permissions(&systemctl, std::fs::Permissions::from_mode(0o755))
        .expect("make fake systemctl executable");
    let _tools = crate::system_tools::use_fake_tool_bin(&root);
    let paths = paths(ServiceManagerKind::Systemd);

    assert!(active_candidate(ServiceManagerKind::Systemd, &paths).await);
    let active_check = status_check(ServiceManagerKind::Systemd, &paths).await;
    assert_eq!(active_check.severity, DoctorSeverity::Pass);
    assert!(active_check
        .details
        .as_deref()
        .is_some_and(|details| details.contains("State: LoadState=loaded")));

    std::fs::write(
        &systemctl,
        "#!/bin/sh\nprintf 'LoadState=loaded\\nActiveState=inactive\\nSubState=dead\\n'\n",
    )
    .expect("write inactive systemctl probe");
    assert!(!active_candidate(ServiceManagerKind::Systemd, &paths).await);
    let inactive_check = status_check(ServiceManagerKind::Systemd, &paths).await;
    assert_eq!(inactive_check.severity, DoctorSeverity::Warning);
    assert!(inactive_check
        .details
        .as_deref()
        .is_some_and(|details| details.contains("State: LoadState=loaded")));

    std::fs::remove_dir_all(root).expect("remove fake tool directory");
}
