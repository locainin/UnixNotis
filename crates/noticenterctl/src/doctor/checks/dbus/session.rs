//! Session connection, identity, and daemon-proxy probes

use unixnotis_core::log_session_bus_identity;
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::super::super::report::safe_doctor_text;
use super::super::super::report::{DoctorCheck, DoctorSeverity};
use super::DBUS_CHECK_TIMEOUT;

pub(super) struct SessionProbe {
    pub(super) connection: Option<Connection>,
    pub(super) checks: Vec<DoctorCheck>,
}

pub(super) async fn probe_session() -> SessionProbe {
    // A broken session environment must never make doctor hang
    match tokio::time::timeout(DBUS_CHECK_TIMEOUT, Connection::session()).await {
        Ok(Ok(connection)) => SessionProbe {
            connection: Some(connection),
            checks: Vec::new(),
        },
        Ok(Err(error)) => unavailable_probe(format!("Session bus connection failed: {error}")),
        Err(_) => unavailable_probe("Session bus connection timed out".to_string()),
    }
}

pub(super) async fn connected_checks(connection: &Connection) -> Vec<DoctorCheck> {
    let mut checks = vec![DoctorCheck::new(
        "dbus.session",
        "Session bus",
        DoctorSeverity::Pass,
        "Session bus connection succeeded",
    )];
    let identity_check = match log_session_bus_identity(connection, "noticenterctl doctor").await {
        Ok(identity) => DoctorCheck::new(
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
        Err(error) => DoctorCheck::new(
            "dbus.identity",
            "Session bus identity",
            DoctorSeverity::Error,
            "Session bus identity probe failed",
        )
        .details(safe_doctor_text(&error.to_string())),
    };
    checks.push(identity_check);
    checks
}

pub(super) async fn build_bus_proxy(connection: &Connection) -> Result<DBusProxy<'_>, DoctorCheck> {
    match tokio::time::timeout(DBUS_CHECK_TIMEOUT, DBusProxy::new(connection)).await {
        Ok(Ok(proxy)) => Ok(proxy),
        Ok(Err(error)) => Err(DoctorCheck::new(
            "dbus.proxy",
            "Session bus proxy",
            DoctorSeverity::Error,
            "D-Bus daemon proxy construction failed",
        )
        .details(safe_doctor_text(&error.to_string()))),
        Err(_) => Err(DoctorCheck::new(
            "dbus.proxy",
            "Session bus proxy",
            DoctorSeverity::Error,
            "D-Bus daemon proxy construction timed out",
        )),
    }
}

pub(super) fn unavailable_probe(details: String) -> SessionProbe {
    // Dependent checks collapse into one note instead of cascading misleading failures
    SessionProbe {
        connection: None,
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
    }
}
