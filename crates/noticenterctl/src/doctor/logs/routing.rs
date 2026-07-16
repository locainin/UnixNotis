//! Backend-aware log source selection

use super::super::report::{DoctorLogResult, DoctorLogSource};
use super::super::service::SelectedServiceManager;

const LOGGING_HINT: &str =
    "Reproduce the problem by running unixnotis-daemon in a terminal, or configure logging through the service manager";

pub(in crate::doctor) async fn collect_logs(
    selected: SelectedServiceManager,
    verbose: bool,
) -> DoctorLogResult {
    // Only the installed systemd backend provides a configured persistent source
    match selected {
        SelectedServiceManager::Managed(
            unixnotis_core::service_manager::ServiceManagerKind::Systemd,
        ) => super::systemd::collect_systemd_logs(verbose).await,
        SelectedServiceManager::Managed(manager) => unavailable_manager_logs(manager.label()),
        SelectedServiceManager::Manual => unavailable_logs(
            DoctorLogSource::Manual,
            "Manual launches do not provide a UnixNotis-managed persistent log source",
        ),
        SelectedServiceManager::Unknown => unavailable_logs(
            DoctorLogSource::Unknown,
            "The active service manager is unknown, so no persistent log source can be selected safely",
        ),
    }
}

pub(super) fn unavailable_manager_logs(manager: &str) -> DoctorLogResult {
    // Backend identity remains explicit even when no log source exists
    let source = match manager {
        "dinit" => DoctorLogSource::Dinit,
        "runit" => DoctorLogSource::Runit,
        "s6-rc" => DoctorLogSource::S6Rc,
        _ => DoctorLogSource::Unknown,
    };
    unavailable_logs(
        source,
        &format!(
            "Persistent logs unavailable: the installed UnixNotis {manager} service does not configure a manager logger"
        ),
    )
}

pub(super) fn unavailable_logs(source: DoctorLogSource, reason: &str) -> DoctorLogResult {
    // Missing backend logs are informational and never change doctor exit status
    DoctorLogResult::Unavailable {
        source,
        reason: reason.to_string(),
        hint: Some(LOGGING_HINT.to_string()),
    }
}
