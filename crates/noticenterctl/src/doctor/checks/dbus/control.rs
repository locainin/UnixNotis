//! Control proxy, state, and composite UI readiness checks

use unixnotis_core::ControlProxy;
use zbus::Connection;

use super::super::super::report::safe_doctor_text;
use super::super::super::report::{DoctorCheck, DoctorSeverity};
use super::classify::control_state_failure_check;
use super::DBUS_CHECK_TIMEOUT;

pub(super) async fn inspect_control(connection: &Connection) -> Vec<DoctorCheck> {
    let mut checks = Vec::new();
    let control =
        match tokio::time::timeout(DBUS_CHECK_TIMEOUT, ControlProxy::new(connection)).await {
            Ok(Ok(proxy)) => proxy,
            Ok(Err(error)) => {
                checks.push(
                    DoctorCheck::new(
                        "dbus.control-proxy",
                        "UnixNotis control proxy",
                        DoctorSeverity::Error,
                        "Control proxy construction failed",
                    )
                    .details(safe_doctor_text(&error.to_string())),
                );
                return checks;
            }
            Err(_) => {
                checks.push(DoctorCheck::new(
                    "dbus.control-proxy",
                    "UnixNotis control proxy",
                    DoctorSeverity::Error,
                    "Control proxy construction timed out",
                ));
                return checks;
            }
        };
    checks.push(DoctorCheck::new(
        "dbus.control-proxy",
        "UnixNotis control proxy",
        DoctorSeverity::Pass,
        "Control proxy construction succeeded",
    ));

    checks.push(inspect_control_state(&control).await);
    checks.push(inspect_ui_health(&control).await);
    checks
}

pub(super) fn unavailable_control_check() -> DoctorCheck {
    DoctorCheck::new(
        "dbus.control-state",
        "UnixNotis control state",
        DoctorSeverity::Error,
        "UnixNotis control service has no owner",
    )
    .hint("Check the selected service manager status below")
}

async fn inspect_control_state(control: &ControlProxy<'_>) -> DoctorCheck {
    match tokio::time::timeout(DBUS_CHECK_TIMEOUT, control.get_state()).await {
        Ok(Ok(state)) => DoctorCheck::new(
            "dbus.control-state",
            "UnixNotis control state",
            DoctorSeverity::Pass,
            "GetState completed",
        )
        .details(format!(
            "DND: {}\nHistory entries: {}\nInhibitors: {}",
            state.dnd_enabled, state.history_count, state.inhibitor_count
        ))
        .data("dnd_enabled", state.dnd_enabled)
        .data("history_count", state.history_count)
        .data("inhibitor_count", state.inhibitor_count),
        Ok(Err(error)) => control_state_failure_check(&error),
        Err(_) => DoctorCheck::new(
            "dbus.control-state",
            "UnixNotis control state",
            DoctorSeverity::Error,
            "GetState timed out",
        ),
    }
}

async fn inspect_ui_health(control: &ControlProxy<'_>) -> DoctorCheck {
    match tokio::time::timeout(DBUS_CHECK_TIMEOUT, control.get_ui_health()).await {
        Ok(Ok(health)) => {
            let healthy = health.center_process_running
                && health.center_ready
                && health.popups_process_running
                && health.popups_ready;
            DoctorCheck::new(
                "dbus.ui-health",
                "UnixNotis UI readiness",
                if healthy {
                    DoctorSeverity::Pass
                } else {
                    DoctorSeverity::Error
                },
                if healthy {
                    "Center and popup clients are ready"
                } else {
                    "One or more UI clients are not ready"
                },
            )
            .details(format!(
                "Center process: {}\nCenter D-Bus client: {}\nPopup process: {}\nPopup D-Bus/GTK client: {}",
                readiness_label(health.center_process_running),
                readiness_label(health.center_ready),
                readiness_label(health.popups_process_running),
                readiness_label(health.popups_ready),
            ))
            .data("center_process_running", health.center_process_running)
            .data("center_ready", health.center_ready)
            .data("popups_process_running", health.popups_process_running)
            .data("popups_ready", health.popups_ready)
        }
        Ok(Err(error)) => DoctorCheck::new(
            "dbus.ui-health",
            "UnixNotis UI readiness",
            DoctorSeverity::Error,
            "GetUiHealth failed",
        )
        .details(safe_doctor_text(&error.to_string())),
        Err(_) => DoctorCheck::new(
            "dbus.ui-health",
            "UnixNotis UI readiness",
            DoctorSeverity::Error,
            "GetUiHealth timed out",
        ),
    }
}

const fn readiness_label(ready: bool) -> &'static str {
    if ready {
        "ready"
    } else {
        "not ready"
    }
}
