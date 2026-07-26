//! Session validation, manager selection, and backend dispatch

use std::env;

use anyhow::Result;
use unixnotis_core::service_manager::ServiceManagerKind;

use crate::cli::DoctorServiceManagerArg;

use super::backends::{sync_dinit, sync_runit, sync_s6, sync_systemd};
use super::manager::select_manager;
use super::variables::{validate_persisted_bus_address, validate_session_environment};

pub fn sync(requested: DoctorServiceManagerArg) -> Result<()> {
    // Reject detached launches before resolving or mutating service state
    validate_session_environment(|name| env::var_os(name))?;
    let manager = select_manager(requested)?;
    validate_persisted_bus_address(manager.kind, env::var_os("DBUS_SESSION_BUS_ADDRESS"))?;
    // Each backend owns its native restart and environment publication contract
    match manager.kind {
        ServiceManagerKind::Systemd => sync_systemd(),
        ServiceManagerKind::Dinit => sync_dinit(),
        ServiceManagerKind::Runit => sync_runit(&manager),
        ServiceManagerKind::S6 => sync_s6(&manager),
    }
}
