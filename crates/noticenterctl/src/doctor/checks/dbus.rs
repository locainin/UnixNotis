//! Bounded session-bus and `UnixNotis` control checks

use std::time::Duration;

use unixnotis_core::{
    log_session_bus_identity, ControlProxy, CONTROL_BUS_NAME, NOTIFICATIONS_BUS_NAME,
};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::Connection;

use super::super::report::safe_doctor_text;
use super::super::report::{DoctorCheck, DoctorSeverity};

const DBUS_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

pub(in crate::doctor) struct DoctorBusResult {
    pub checks: Vec<DoctorCheck>,
    pub control_owned: bool,
    pub connected: bool,
}

pub(in crate::doctor) async fn inspect_bus() -> DoctorBusResult {
    // Every bus operation is bounded so doctor cannot hang on a broken session
    let connection = match tokio::time::timeout(DBUS_CHECK_TIMEOUT, Connection::session()).await {
        Ok(Ok(connection)) => connection,
        Ok(Err(error)) => {
            return unavailable_bus_result(format!("Session bus connection failed: {error}"));
        }
        Err(_) => return unavailable_bus_result("Session bus connection timed out".to_string()),
    };

    inspect_bus_connection(&connection).await
}

pub(super) async fn inspect_bus_connection(connection: &Connection) -> DoctorBusResult {
    let mut checks = vec![DoctorCheck::new(
        "dbus.session",
        "Session bus",
        DoctorSeverity::Pass,
        "Session bus connection succeeded",
    )];
    match log_session_bus_identity(connection, "noticenterctl doctor").await {
        Ok(identity) => checks.push(
            DoctorCheck::new(
                "dbus.identity",
                "Session bus identity",
                DoctorSeverity::Pass,
                "Session bus identity probe succeeded",
            )
            .details(format!(
                "Bus ID: {}\nUnique name: {}\nRuntime directory: {}",
                identity.bus_id, identity.unique_name, identity.runtime_dir
            ))
            .data("bus_id", identity.bus_id)
            .data("unique_name", identity.unique_name)
            .data("runtime_dir", identity.runtime_dir),
        ),
        Err(error) => checks.push(
            DoctorCheck::new(
                "dbus.identity",
                "Session bus identity",
                DoctorSeverity::Error,
                "Session bus identity probe failed",
            )
            .details(safe_doctor_text(&error.to_string())),
        ),
    }
    // The daemon proxy is required for ownership checks but not for later service checks
    let proxy = match tokio::time::timeout(DBUS_CHECK_TIMEOUT, DBusProxy::new(connection)).await {
        Ok(Ok(proxy)) => proxy,
        Ok(Err(error)) => {
            checks.push(
                DoctorCheck::new(
                    "dbus.proxy",
                    "Session bus proxy",
                    DoctorSeverity::Error,
                    "D-Bus daemon proxy construction failed",
                )
                .details(safe_doctor_text(&error.to_string())),
            );
            return DoctorBusResult {
                checks,
                control_owned: false,
                connected: true,
            };
        }
        Err(_) => {
            checks.push(DoctorCheck::new(
                "dbus.proxy",
                "Session bus proxy",
                DoctorSeverity::Error,
                "D-Bus daemon proxy construction timed out",
            ));
            return DoctorBusResult {
                checks,
                control_owned: false,
                connected: true,
            };
        }
    };

    // Notification and control names are separate readiness signals
    let notifications_owner = check_owner(
        &proxy,
        NOTIFICATIONS_BUS_NAME,
        "dbus.notifications-owner",
        "Notification service",
        &mut checks,
    )
    .await;
    if notifications_owner.is_none() {
        // Missing the standard name means desktop applications have no notification target
        checks.push(
            DoctorCheck::new(
                "dbus.notifications-readiness",
                "Notification readiness",
                DoctorSeverity::Error,
                "No notification service owns org.freedesktop.Notifications",
            )
            .hint("Start unixnotis-daemon and run doctor again"),
        );
    }

    let control_owner = check_owner(
        &proxy,
        CONTROL_BUS_NAME,
        "dbus.control-owner",
        "UnixNotis control service",
        &mut checks,
    )
    .await;
    let has_control_owner = control_owner.is_some();
    if let (Some(notifications_owner), Some(control_owner)) = (&notifications_owner, &control_owner)
    {
        let owners_match = notifications_owner == control_owner;
        checks.push(
            DoctorCheck::new(
                "dbus.shared-owner",
                "UnixNotis D-Bus ownership",
                if owners_match {
                    DoctorSeverity::Pass
                } else {
                    DoctorSeverity::Error
                },
                if owners_match {
                    "Notification and control names share one owner"
                } else {
                    "Notification and control names have different owners"
                },
            )
            .data("notifications_owner", notifications_owner.clone())
            .data("control_owner", control_owner.clone()),
        );
    }
    if has_control_owner {
        // Proxy and GetState checks run only after ownership is confirmed
        inspect_control_proxy(connection, &mut checks).await;
    } else {
        checks.push(
            DoctorCheck::new(
                "dbus.control-state",
                "UnixNotis control state",
                DoctorSeverity::Error,
                "UnixNotis control service has no owner",
            )
            .hint("Check the selected service manager status below"),
        );
    }

    DoctorBusResult {
        checks,
        control_owned: has_control_owner,
        connected: true,
    }
}

async fn check_owner(
    proxy: &DBusProxy<'_>,
    name: &'static str,
    id: &'static str,
    label: &'static str,
    checks: &mut Vec<DoctorCheck>,
) -> Option<String> {
    // Static names are validated here once before the bounded remote request
    let bus_name = BusName::try_from(name).expect("static D-Bus name must be valid");
    match tokio::time::timeout(DBUS_CHECK_TIMEOUT, proxy.name_has_owner(bus_name.clone())).await {
        Ok(Ok(true)) => {
            match tokio::time::timeout(DBUS_CHECK_TIMEOUT, proxy.get_name_owner(bus_name)).await {
                Ok(Ok(owner)) => {
                    let owner = owner.to_string();
                    checks.push(
                        DoctorCheck::new(
                            id,
                            label,
                            DoctorSeverity::Pass,
                            format!("{name} has an owner"),
                        )
                        .data("owner", owner.clone()),
                    );
                    Some(owner)
                }
                Ok(Err(error)) => {
                    checks.push(
                        DoctorCheck::new(
                            id,
                            label,
                            DoctorSeverity::Error,
                            format!("Unable to read {name} owner"),
                        )
                        .details(safe_doctor_text(&error.to_string())),
                    );
                    None
                }
                Err(_) => {
                    checks.push(DoctorCheck::new(
                        id,
                        label,
                        DoctorSeverity::Error,
                        format!("Owner query for {name} timed out"),
                    ));
                    None
                }
            }
        }
        Ok(Ok(false)) => {
            checks.push(DoctorCheck::new(
                id,
                label,
                DoctorSeverity::Warning,
                format!("{name} has no owner"),
            ));
            None
        }
        Ok(Err(error)) => {
            checks.push(
                DoctorCheck::new(
                    id,
                    label,
                    DoctorSeverity::Error,
                    format!("Unable to inspect {name} ownership"),
                )
                .details(safe_doctor_text(&error.to_string())),
            );
            None
        }
        Err(_) => {
            checks.push(DoctorCheck::new(
                id,
                label,
                DoctorSeverity::Error,
                format!("Ownership query for {name} timed out"),
            ));
            None
        }
    }
}

async fn inspect_control_proxy(connection: &Connection, checks: &mut Vec<DoctorCheck>) {
    // Keep proxy construction distinct from GetState for precise failure reports
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
                return;
            }
            Err(_) => {
                checks.push(DoctorCheck::new(
                    "dbus.control-proxy",
                    "UnixNotis control proxy",
                    DoctorSeverity::Error,
                    "Control proxy construction timed out",
                ));
                return;
            }
        };
    checks.push(DoctorCheck::new(
        "dbus.control-proxy",
        "UnixNotis control proxy",
        DoctorSeverity::Pass,
        "Control proxy construction succeeded",
    ));

    // GetState proves that the owner can serve the real control interface
    match tokio::time::timeout(DBUS_CHECK_TIMEOUT, control.get_state()).await {
        Ok(Ok(state)) => checks.push(
            DoctorCheck::new(
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
        ),
        Ok(Err(error)) => checks.push(control_state_failure_check(&error)),
        Err(_) => checks.push(DoctorCheck::new(
            "dbus.control-state",
            "UnixNotis control state",
            DoctorSeverity::Error,
            "GetState timed out",
        )),
    }
}

pub(super) fn control_state_failure_check(error: &zbus::Error) -> DoctorCheck {
    // Access denial is expected when a development binary calls a strict installed daemon
    if control_access_was_denied(error) {
        return DoctorCheck::new(
            "dbus.control-state",
            "UnixNotis control state",
            DoctorSeverity::Error,
            "UnixNotis control access denied",
        )
        .details("The running daemon rejected this client")
        .hint(
            "Use the installed noticenterctl from the same installation as the daemon; uninstalled development binaries are intentionally rejected",
        );
    }

    // Other failures retain the broker detail because they need different troubleshooting
    DoctorCheck::new(
        "dbus.control-state",
        "UnixNotis control state",
        DoctorSeverity::Error,
        "GetState failed",
    )
    .details(safe_doctor_text(&error.to_string()))
}

fn control_access_was_denied(error: &zbus::Error) -> bool {
    match error {
        zbus::Error::MethodError(name, _, _) => {
            name.as_str() == "org.freedesktop.DBus.Error.AccessDenied"
        }
        zbus::Error::FDO(error) => matches!(error.as_ref(), zbus::fdo::Error::AccessDenied(_)),
        _ => false,
    }
}

pub(super) fn unavailable_bus_result(details: String) -> DoctorBusResult {
    // Dependent checks become one note instead of a chain of misleading errors
    DoctorBusResult {
        checks: vec![
            DoctorCheck::new(
                "dbus.session",
                "Session bus",
                DoctorSeverity::Error,
                "Session bus is unavailable",
            )
            .details(safe_doctor_text(&details)),
            DoctorCheck::new(
                "dbus.dependent-checks",
                "D-Bus dependent checks",
                DoctorSeverity::Note,
                "Owner, proxy, and GetState checks could not run",
            ),
        ],
        control_owned: false,
        connected: false,
    }
}
