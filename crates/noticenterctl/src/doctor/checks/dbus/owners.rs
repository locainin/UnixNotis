//! Notification and control name ownership probes

use zbus::fdo::DBusProxy;
use zbus::names::BusName;

use super::super::super::report::safe_doctor_text;
use super::super::super::report::{DoctorCheck, DoctorSeverity};
use super::DBUS_CHECK_TIMEOUT;
use unixnotis_core::{CONTROL_BUS_NAME, NOTIFICATIONS_BUS_NAME};

#[derive(Debug)]
pub(super) enum OwnerState {
    Owned(String),
    Unowned,
    QueryFailed,
    TimedOut,
}

pub(super) struct OwnerProbe {
    pub(super) state: OwnerState,
    pub(super) check: DoctorCheck,
}

impl OwnerProbe {
    pub(super) fn owner(&self) -> Option<&str> {
        match &self.state {
            OwnerState::Owned(owner) => Some(owner),
            OwnerState::Unowned | OwnerState::QueryFailed | OwnerState::TimedOut => None,
        }
    }
}

pub(super) async fn probe_notifications_owner(proxy: &DBusProxy<'_>) -> OwnerProbe {
    probe_owner(
        proxy,
        NOTIFICATIONS_BUS_NAME,
        "dbus.notifications-owner",
        "Notification service",
    )
    .await
}

pub(super) async fn probe_control_owner(proxy: &DBusProxy<'_>) -> OwnerProbe {
    probe_owner(
        proxy,
        CONTROL_BUS_NAME,
        "dbus.control-owner",
        "UnixNotis control service",
    )
    .await
}

async fn probe_owner(
    proxy: &DBusProxy<'_>,
    name: &'static str,
    id: &'static str,
    label: &'static str,
) -> OwnerProbe {
    let bus_name = BusName::try_from(name).expect("static D-Bus name must be valid");
    match tokio::time::timeout(DBUS_CHECK_TIMEOUT, proxy.name_has_owner(bus_name.clone())).await {
        Ok(Ok(true)) => read_owner(proxy, bus_name, name, id, label).await,
        Ok(Ok(false)) => OwnerProbe {
            state: OwnerState::Unowned,
            check: DoctorCheck::new(
                id,
                label,
                DoctorSeverity::Warning,
                format!("{name} has no owner"),
            ),
        },
        Ok(Err(error)) => OwnerProbe {
            state: OwnerState::QueryFailed,
            check: DoctorCheck::new(
                id,
                label,
                DoctorSeverity::Error,
                format!("Unable to inspect {name} ownership"),
            )
            .details(safe_doctor_text(&error.to_string())),
        },
        Err(_) => OwnerProbe {
            state: OwnerState::TimedOut,
            check: DoctorCheck::new(
                id,
                label,
                DoctorSeverity::Error,
                format!("Ownership query for {name} timed out"),
            ),
        },
    }
}

async fn read_owner(
    proxy: &DBusProxy<'_>,
    bus_name: BusName<'_>,
    name: &'static str,
    id: &'static str,
    label: &'static str,
) -> OwnerProbe {
    match tokio::time::timeout(DBUS_CHECK_TIMEOUT, proxy.get_name_owner(bus_name)).await {
        Ok(Ok(owner)) => {
            let owner = owner.to_string();
            OwnerProbe {
                state: OwnerState::Owned(owner.clone()),
                check: DoctorCheck::new(
                    id,
                    label,
                    DoctorSeverity::Pass,
                    format!("{name} has an owner"),
                )
                .data("owner", owner),
            }
        }
        Ok(Err(error)) => OwnerProbe {
            state: OwnerState::QueryFailed,
            check: DoctorCheck::new(
                id,
                label,
                DoctorSeverity::Error,
                format!("Unable to read {name} owner"),
            )
            .details(safe_doctor_text(&error.to_string())),
        },
        Err(_) => OwnerProbe {
            state: OwnerState::TimedOut,
            check: DoctorCheck::new(
                id,
                label,
                DoctorSeverity::Error,
                format!("Owner query for {name} timed out"),
            ),
        },
    }
}

pub(super) fn notification_readiness_failure() -> DoctorCheck {
    DoctorCheck::new(
        "dbus.notifications-readiness",
        "Notification readiness",
        DoctorSeverity::Error,
        "No notification service owns org.freedesktop.Notifications",
    )
    .hint("Start unixnotis-daemon and run doctor again")
}

pub(super) fn shared_owner_check(notification_owner: &str, control_owner: &str) -> DoctorCheck {
    let owners_match = notification_owner == control_owner;
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
    .data("notifications_owner", notification_owner)
    .data("control_owner", control_owner)
}
