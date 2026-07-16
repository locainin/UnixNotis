use std::ffi::OsString;

use std::path::PathBuf;

use unixnotis_core::service_manager::{ServiceManagerKind, ServiceManagerPaths};

use super::super::selection::{
    add_stale_artifact_check, collect_evidence, explicit_selection, manager_from_environment,
    probe_error_check, select_from_evidence, ServiceEvidence,
};
use super::super::SelectedServiceManager;
use crate::cli::DoctorServiceManagerArg;

fn evidence(kind: ServiceManagerKind, artifact_present: bool, active: bool) -> ServiceEvidence {
    ServiceEvidence {
        kind,
        artifact_present,
        active,
        probe_error: None,
    }
}

#[test]
fn active_runtime_evidence_outranks_stale_artifacts() {
    let evidence = [
        evidence(ServiceManagerKind::Systemd, true, false),
        evidence(ServiceManagerKind::Dinit, false, true),
    ];
    let mut checks = Vec::new();

    let selected = select_from_evidence(&evidence, true, &mut checks);

    assert_eq!(
        selected,
        SelectedServiceManager::Managed(ServiceManagerKind::Dinit)
    );
}

#[test]
fn reachable_control_with_only_stale_artifacts_is_manual() {
    let evidence = [evidence(ServiceManagerKind::Systemd, true, false)];
    let mut checks = Vec::new();

    let selected = select_from_evidence(&evidence, true, &mut checks);

    assert_eq!(selected, SelectedServiceManager::Manual);
}

#[test]
fn one_artifact_is_only_probable_when_control_is_unavailable() {
    let evidence = [evidence(ServiceManagerKind::Runit, true, false)];
    let mut checks = Vec::new();

    let selected = select_from_evidence(&evidence, false, &mut checks);

    assert_eq!(
        selected,
        SelectedServiceManager::Managed(ServiceManagerKind::Runit)
    );
    assert!(checks.iter().any(|check| check.id == "service.selection"));
}

#[test]
fn environment_selection_is_strict_and_empty_means_no_override() {
    assert_eq!(manager_from_environment(None).expect("missing value"), None);
    assert_eq!(
        manager_from_environment(Some(OsString::new())).expect("empty value"),
        None
    );
    assert_eq!(
        manager_from_environment(Some(OsString::from("dinit"))).expect("known value"),
        Some(SelectedServiceManager::Managed(ServiceManagerKind::Dinit))
    );
    assert!(manager_from_environment(Some(OsString::from("unsupported"))).is_err());
}

#[test]
fn invalid_environment_selection_is_sanitized_redacted_and_bounded() {
    let home = std::env::var("HOME").expect("HOME");
    let raw = format!("{home}/\u{1b}[31m{}", "x".repeat(2_000));

    let check = manager_from_environment(Some(OsString::from(raw)))
        .expect_err("invalid manager must produce a check");
    let details = check.details.expect("invalid manager details");

    assert!(!details.contains(&home));
    assert!(!details.contains('\u{1b}'));
    assert!(details.chars().count() <= crate::doctor::report::DOCTOR_DETAIL_CHAR_LIMIT);
}

#[test]
fn every_explicit_service_manager_selection_bypasses_auto_detection() {
    let cases = [
        (
            DoctorServiceManagerArg::Systemd,
            SelectedServiceManager::Managed(ServiceManagerKind::Systemd),
        ),
        (
            DoctorServiceManagerArg::Dinit,
            SelectedServiceManager::Managed(ServiceManagerKind::Dinit),
        ),
        (
            DoctorServiceManagerArg::Runit,
            SelectedServiceManager::Managed(ServiceManagerKind::Runit),
        ),
        (
            DoctorServiceManagerArg::S6,
            SelectedServiceManager::Managed(ServiceManagerKind::S6),
        ),
        (
            DoctorServiceManagerArg::Manual,
            SelectedServiceManager::Manual,
        ),
    ];

    for (request, expected) in cases {
        assert_eq!(explicit_selection(request), Some(expected));
    }
    assert_eq!(explicit_selection(DoctorServiceManagerArg::Auto), None);
}

#[test]
fn multiple_active_managers_are_reported_as_ambiguous() {
    let evidence = [
        evidence(ServiceManagerKind::Systemd, true, true),
        evidence(ServiceManagerKind::Dinit, true, true),
    ];
    let mut checks = Vec::new();

    let selected = select_from_evidence(&evidence, true, &mut checks);

    assert_eq!(selected, SelectedServiceManager::Unknown);
    assert!(checks.iter().any(|check| {
        check.id == "service.selection" && check.summary.contains("Multiple service managers")
    }));
}

#[test]
fn multiple_inactive_artifacts_are_reported_as_ambiguous() {
    let evidence = [
        evidence(ServiceManagerKind::Runit, true, false),
        evidence(ServiceManagerKind::S6, true, false),
    ];
    let mut checks = Vec::new();

    let selected = select_from_evidence(&evidence, false, &mut checks);

    assert_eq!(selected, SelectedServiceManager::Unknown);
    assert!(checks.iter().any(|check| {
        check.id == "service.selection" && check.summary.contains("Multiple inactive")
    }));
}

#[test]
fn stale_artifact_warning_excludes_the_selected_and_active_backends() {
    let evidence = [
        evidence(ServiceManagerKind::Systemd, true, false),
        evidence(ServiceManagerKind::Dinit, true, true),
        evidence(ServiceManagerKind::Runit, false, false),
    ];
    let mut checks = Vec::new();

    add_stale_artifact_check(
        &evidence,
        SelectedServiceManager::Managed(ServiceManagerKind::Dinit),
        &mut checks,
    );

    let stale = checks
        .iter()
        .find(|check| check.id == "service.stale-artifacts")
        .expect("stale artifact warning");
    assert_eq!(stale.details.as_deref(), Some("systemd"));
}

#[test]
fn probe_error_note_exists_only_for_relevant_failed_backends() {
    let unavailable = ServiceEvidence {
        kind: ServiceManagerKind::Runit,
        artifact_present: true,
        active: false,
        probe_error: Some("sv unavailable".to_string()),
    };
    let healthy = evidence(ServiceManagerKind::Dinit, true, true);

    let check = probe_error_check(&[unavailable]).expect("probe error note");
    assert_eq!(check.id, "service.probe-errors");
    assert!(check
        .details
        .as_deref()
        .is_some_and(|detail| detail.contains("runit: sv unavailable")));
    assert!(probe_error_check(&[healthy]).is_none());
}

#[tokio::test]
async fn resolved_evidence_collection_returns_each_supported_manager_in_order() {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-doctor-empty-tools-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create empty fake tool directory");
    let tools = crate::system_tools::use_fake_tool_bin(&root);

    let resolved = ServiceManagerKind::all()
        .into_iter()
        .map(|kind| {
            let artifact_root = root.join(kind.label());
            let live_root = (kind == ServiceManagerKind::S6).then(|| root.join("s6-live"));
            (
                kind,
                Ok(ServiceManagerPaths {
                    kind,
                    artifact_root,
                    live_root,
                }),
            )
        })
        .collect();

    let (evidence, checks) = collect_evidence(resolved).await;

    drop(tools);
    let _ = std::fs::remove_dir_all(root);
    assert_eq!(evidence.len(), ServiceManagerKind::all().len());
    assert_eq!(
        evidence.iter().map(|item| item.kind).collect::<Vec<_>>(),
        ServiceManagerKind::all()
    );
    assert!(
        checks.is_empty(),
        "fully resolved inactive managers should not create path warnings"
    );
}

#[tokio::test]
async fn unresolved_manager_path_is_reported_without_hiding_resolved_managers() {
    let resolved = vec![
        (
            ServiceManagerKind::Systemd,
            Ok(ServiceManagerPaths {
                kind: ServiceManagerKind::Systemd,
                artifact_root: PathBuf::from("/tmp/systemd-user"),
                live_root: None,
            }),
        ),
        (ServiceManagerKind::S6, Err("USER is not set".to_string())),
    ];

    let (evidence, checks) = collect_evidence(resolved).await;

    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].kind, ServiceManagerKind::Systemd);
    let warning = checks
        .iter()
        .find(|check| check.id == "service.path-inspection")
        .expect("path inspection warning");
    assert!(
        warning
            .details
            .as_deref()
            .is_some_and(|details| details.contains("s6-rc: USER is not set")),
        "path warning should identify the omitted manager"
    );
}
