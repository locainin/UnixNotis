use unixnotis_core::service_manager::ServiceManagerKind;

use super::super::model::SelectedServiceManager;

#[test]
fn selected_manager_distinguishes_managed_manual_and_unknown_sources() {
    assert_ne!(
        SelectedServiceManager::Managed(ServiceManagerKind::Systemd),
        SelectedServiceManager::Manual
    );
    assert_ne!(
        SelectedServiceManager::Manual,
        SelectedServiceManager::Unknown
    );
}
