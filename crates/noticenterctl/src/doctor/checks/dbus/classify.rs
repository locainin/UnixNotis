//! Stable user-facing classification for control-call failures

use super::super::super::report::safe_doctor_text;
use super::super::super::report::{DoctorCheck, DoctorSeverity};

pub(super) fn control_state_failure_check(error: &zbus::Error) -> DoctorCheck {
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
