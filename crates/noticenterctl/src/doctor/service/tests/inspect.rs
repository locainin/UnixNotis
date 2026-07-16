use crate::cli::DoctorServiceManagerArg;

use super::super::inspect::inspect_service_manager;
use super::super::model::SelectedServiceManager;

#[tokio::test]
async fn explicit_manual_selection_reports_dbus_reachability_without_manager_commands() {
    let result = inspect_service_manager(DoctorServiceManagerArg::Manual, true).await;

    assert_eq!(result.selected, SelectedServiceManager::Manual);
    assert!(result.checks.iter().any(|check| {
        check.id == "service.status" && check.summary == "Manual launch is reachable through D-Bus"
    }));
}
