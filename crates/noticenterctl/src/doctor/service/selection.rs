//! Evidence-based service-manager selection policy

use std::env;

use unixnotis_core::service_manager::{
    resolve_service_manager_paths, ServiceManagerKind, ServiceManagerPaths,
};

use crate::cli::DoctorServiceManagerArg;

use super::super::report::safe_doctor_text;
use super::super::report::{DoctorCheck, DoctorSeverity};
use super::artifacts::primary_artifact;
use super::model::SelectedServiceManager;
use super::probe::active_candidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ServiceEvidence {
    pub kind: ServiceManagerKind,
    pub artifact_present: bool,
    pub active: bool,
    pub probe_error: Option<String>,
}

pub(super) struct ServiceSelection {
    pub selected: SelectedServiceManager,
    pub checks: Vec<DoctorCheck>,
}

pub(super) async fn select_service_manager(
    requested: DoctorServiceManagerArg,
    control_owned: bool,
) -> ServiceSelection {
    // Explicit CLI input has priority because it represents a deliberate diagnosis target
    if let Some(selected) = explicit_selection(requested) {
        return ServiceSelection {
            selected,
            checks: Vec::new(),
        };
    }
    // The installer environment contract is the next strongest identity signal
    match manager_from_environment(env::var_os("UNIXNOTIS_SERVICE_MANAGER")) {
        Ok(Some(selected)) => {
            return ServiceSelection {
                selected,
                checks: Vec::new(),
            };
        }
        Err(check) => {
            return ServiceSelection {
                selected: SelectedServiceManager::Unknown,
                checks: vec![*check],
            };
        }
        Ok(None) => {}
    }

    // Auto mode compares live probes and installed artifacts instead of guessing by platform
    let (evidence, mut checks) = collect_evidence().await;
    let selected = select_from_evidence(&evidence, control_owned, &mut checks);
    add_stale_artifact_check(&evidence, selected, &mut checks);
    ServiceSelection { selected, checks }
}

pub(super) const fn explicit_selection(
    requested: DoctorServiceManagerArg,
) -> Option<SelectedServiceManager> {
    match requested {
        DoctorServiceManagerArg::Systemd => {
            Some(SelectedServiceManager::Managed(ServiceManagerKind::Systemd))
        }
        DoctorServiceManagerArg::Dinit => {
            Some(SelectedServiceManager::Managed(ServiceManagerKind::Dinit))
        }
        DoctorServiceManagerArg::Runit => {
            Some(SelectedServiceManager::Managed(ServiceManagerKind::Runit))
        }
        DoctorServiceManagerArg::S6 => {
            Some(SelectedServiceManager::Managed(ServiceManagerKind::S6))
        }
        DoctorServiceManagerArg::Manual => Some(SelectedServiceManager::Manual),
        DoctorServiceManagerArg::Auto => None,
    }
}

pub(super) fn manager_from_environment(
    raw: Option<std::ffi::OsString>,
) -> Result<Option<SelectedServiceManager>, Box<DoctorCheck>> {
    // An absent or empty override leaves normal evidence-based selection enabled
    let Some(raw) = raw.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let raw = raw.to_string_lossy();
    ServiceManagerKind::parse(&raw)
        .map(SelectedServiceManager::Managed)
        .map(Some)
        .map_err(|error| {
            Box::new(
                DoctorCheck::new(
                    "service.selection",
                    "Service",
                    DoctorSeverity::Error,
                    "UNIXNOTIS_SERVICE_MANAGER is invalid",
                )
                .details(safe_doctor_text(&error.to_string())),
            )
        })
}

pub(super) async fn collect_evidence() -> (Vec<ServiceEvidence>, Vec<DoctorCheck>) {
    let mut evidence = Vec::new();
    let mut path_errors = Vec::new();
    // Stable manager order keeps human and JSON reports deterministic
    for kind in ServiceManagerKind::all() {
        match resolve_service_manager_paths(kind) {
            Ok(paths) => evidence.push(evidence_for_paths(kind, &paths).await),
            Err(error) => path_errors.push(format!("{}: {error}", kind.label())),
        }
    }
    let mut checks = if path_errors.is_empty() {
        Vec::new()
    } else {
        vec![DoctorCheck::new(
            "service.path-inspection",
            "Service",
            DoctorSeverity::Warning,
            "Some service-manager paths could not be inspected",
        )
        .details(safe_doctor_text(&path_errors.join("\n")))]
    };
    if let Some(check) = probe_error_check(&evidence) {
        checks.push(check);
    }
    (evidence, checks)
}

pub(super) fn probe_error_check(evidence: &[ServiceEvidence]) -> Option<DoctorCheck> {
    // Missing tools are reported only when an artifact makes that backend relevant
    let probe_errors = evidence
        .iter()
        .filter_map(|item| {
            item.probe_error
                .as_deref()
                .map(|error| format!("{}: {error}", item.kind.label()))
        })
        .collect::<Vec<_>>();
    if probe_errors.is_empty() {
        None
    } else {
        Some(
            DoctorCheck::new(
                "service.probe-errors",
                "Service",
                DoctorSeverity::Note,
                "Some service-manager active probes were unavailable",
            )
            .details(safe_doctor_text(&probe_errors.join("\n"))),
        )
    }
}

async fn evidence_for_paths(
    kind: ServiceManagerKind,
    paths: &ServiceManagerPaths,
) -> ServiceEvidence {
    // Artifact state and runtime state stay separate so stale installs remain visible
    let artifact_present = primary_artifact(paths).is_file();
    let (active, probe_error) = active_candidate(kind, paths).await;
    ServiceEvidence {
        kind,
        artifact_present,
        active,
        // A missing tool is expected when its manager is not installed
        //
        // Probe failures remain useful when an artifact says that the backend
        // was installed, but absent backends should not create diagnostic noise
        probe_error: artifact_present.then_some(probe_error).flatten(),
    }
}

pub(super) fn select_from_evidence(
    evidence: &[ServiceEvidence],
    control_owned: bool,
    checks: &mut Vec<DoctorCheck>,
) -> SelectedServiceManager {
    // A single active native probe is stronger than every filesystem artifact
    let active = evidence
        .iter()
        .filter(|item| item.active)
        .collect::<Vec<_>>();
    if let [item] = active.as_slice() {
        return SelectedServiceManager::Managed(item.kind);
    }
    if active.len() > 1 {
        checks.push(ambiguity_check(
            "Multiple service managers report UnixNotis as active",
            active.iter().map(|item| item.kind),
        ));
        return SelectedServiceManager::Unknown;
    }
    // A healthy control owner without an active managed backend indicates a manual launch
    if control_owned {
        return SelectedServiceManager::Manual;
    }

    // Inactive artifacts are only a fallback when the daemon cannot identify itself
    let artifacts = evidence
        .iter()
        .filter(|item| item.artifact_present)
        .collect::<Vec<_>>();
    if let [item] = artifacts.as_slice() {
        checks.push(
            DoctorCheck::new(
                "service.selection",
                "Service",
                DoctorSeverity::Note,
                "Service manager inferred from one inactive installed artifact",
            )
            .details(format!("Manager: {}", item.kind.label()))
            .data("manager", item.kind.label())
            .hint("The D-Bus control service is unavailable, so this selection is only probable"),
        );
        return SelectedServiceManager::Managed(item.kind);
    }
    if !artifacts.is_empty() {
        checks.push(ambiguity_check(
            "Multiple inactive UnixNotis service artifacts were found",
            artifacts.iter().map(|item| item.kind),
        ));
    }
    SelectedServiceManager::Unknown
}

fn ambiguity_check(
    summary: &'static str,
    managers: impl Iterator<Item = ServiceManagerKind>,
) -> DoctorCheck {
    // Candidate order follows the stable evidence order used by collection
    let labels = managers
        .map(ServiceManagerKind::label)
        .collect::<Vec<_>>()
        .join(", ");
    DoctorCheck::new(
        "service.selection",
        "Service",
        DoctorSeverity::Warning,
        summary,
    )
    .details(labels.clone())
    .data("candidates", labels)
    .hint("Pass --service-manager to select the active backend")
}

pub(super) fn add_stale_artifact_check(
    evidence: &[ServiceEvidence],
    selected: SelectedServiceManager,
    checks: &mut Vec<DoctorCheck>,
) {
    // The selected artifact is not stale merely because its daemon is currently stopped
    let stale = evidence
        .iter()
        .filter(|item| {
            item.artifact_present
                && !item.active
                && selected != SelectedServiceManager::Managed(item.kind)
        })
        .map(|item| item.kind.label())
        .collect::<Vec<_>>();
    // Avoid adding an empty warning that could change an otherwise healthy report
    if stale.is_empty() {
        return;
    }
    let labels = stale.join(", ");
    checks.push(
        DoctorCheck::new(
            "service.stale-artifacts",
            "Service",
            DoctorSeverity::Warning,
            "Inactive service artifacts were found",
        )
        .details(labels.clone())
        .data("managers", labels)
        .hint("Remove artifacts for service managers that are no longer used"),
    );
}
