use super::super::routing::*;
use crate::doctor::report::{DoctorLogResult, DoctorLogSource};
use crate::doctor::service::SelectedServiceManager;
use std::os::unix::fs::PermissionsExt;
use unixnotis_core::service_manager::ServiceManagerKind;

#[test]
fn non_systemd_backends_report_informational_unavailable_logs() {
    for (label, source) in [
        ("dinit", DoctorLogSource::Dinit),
        ("runit", DoctorLogSource::Runit),
        ("s6-rc", DoctorLogSource::S6Rc),
    ] {
        let result = unavailable_manager_logs(label);
        assert!(matches!(
            result,
            DoctorLogResult::Unavailable {
                source: actual,
                reason,
                ..
            } if actual == source && reason.contains(label)
        ));
    }
}

#[test]
fn unavailable_logs_remain_a_tagged_informational_result() {
    assert!(matches!(
        unavailable_logs(DoctorLogSource::Manual, "not configured"),
        DoctorLogResult::Unavailable {
            source: DoctorLogSource::Manual,
            ..
        }
    ));
}

#[tokio::test]
async fn non_systemd_log_collection_executes_no_logger_command() {
    let root =
        std::env::temp_dir().join(format!("unixnotis-doctor-no-logger-{}", std::process::id()));
    let marker = root.join("called");
    std::fs::create_dir_all(&root).expect("create fake tool directory");
    for tool in ["journalctl", "dinitctl", "sv", "s6-log"] {
        let path = root.join(tool);
        std::fs::write(&path, format!("#!/bin/sh\ntouch '{}'\n", marker.display()))
            .expect("write fake logger command");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make fake logger executable");
    }
    let _tools = crate::system_tools::use_fake_tool_bin(&root);

    for manager in [
        ServiceManagerKind::Dinit,
        ServiceManagerKind::Runit,
        ServiceManagerKind::S6,
    ] {
        let result = collect_logs(SelectedServiceManager::Managed(manager), true).await;
        assert!(matches!(result, DoctorLogResult::Unavailable { .. }));
    }

    assert!(!marker.exists());
    std::fs::remove_dir_all(root).expect("remove fake tool directory");
}
