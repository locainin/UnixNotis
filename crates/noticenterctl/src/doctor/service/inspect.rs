//! Selected service-manager artifact and runtime inspection

use unixnotis_core::service_manager::resolve_service_manager_paths;

use crate::cli::DoctorServiceManagerArg;

use super::super::report::safe_doctor_text;
use super::super::report::{DoctorCheck, DoctorSeverity};
use super::artifacts::artifact_check;
use super::model::{DoctorServiceResult, SelectedServiceManager};
use super::probe::status_check;
use super::selection::select_service_manager;

pub(in crate::doctor) async fn inspect_service_manager(
    requested: DoctorServiceManagerArg,
    control_owned: bool,
) -> DoctorServiceResult {
    // Selection diagnostics remain first so later checks have clear manager context
    let selection = select_service_manager(requested, control_owned).await;
    let mut checks = selection.checks;
    let selected = selection.selected;

    // Managed backends inspect both the installed artifact and live supervisor state
    match selected {
        SelectedServiceManager::Managed(kind) => match resolve_service_manager_paths(kind) {
            Ok(paths) => {
                checks.push(artifact_check(&paths));
                checks.push(status_check(kind, &paths).await);
            }
            Err(error) => checks.push(
                DoctorCheck::new(
                    "service.paths",
                    "Service",
                    DoctorSeverity::Error,
                    "Service-manager paths could not be resolved",
                )
                .details(safe_doctor_text(&error.to_string())),
            ),
        },
        // Manual and unknown launches never trigger speculative manager commands
        SelectedServiceManager::Manual => checks.push(manual_status_check(control_owned)),
        SelectedServiceManager::Unknown => checks.push(unknown_status_check()),
    }

    DoctorServiceResult { selected, checks }
}

fn manual_status_check(control_owned: bool) -> DoctorCheck {
    // The control owner is the only objective runtime signal for manual launches
    DoctorCheck::new(
        "service.status",
        "Service",
        if control_owned {
            DoctorSeverity::Pass
        } else {
            DoctorSeverity::Warning
        },
        if control_owned {
            "Manual launch is reachable through D-Bus"
        } else {
            "Manual launch is not reachable through D-Bus"
        },
    )
    .details("Manager: manual")
    .data("manager", "manual")
    .data("control_owned", control_owned)
}

fn unknown_status_check() -> DoctorCheck {
    // Unknown selection is informational because other doctor checks may still be healthy
    DoctorCheck::new(
        "service.status",
        "Service",
        DoctorSeverity::Note,
        "No service manager was selected",
    )
    .details("Manager: unknown")
    .data("manager", "unknown")
    .hint("Pass --service-manager when UnixNotis uses a supported manager")
}
