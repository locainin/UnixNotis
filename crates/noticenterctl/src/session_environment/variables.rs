//! Session variables shared by service-manager backends

use std::ffi::OsString;

use anyhow::{bail, Result};
use unixnotis_core::service_manager::{
    validate_session_bus_address, variables_for_backend, ServiceManagerKind,
};

pub(super) const fn import_variables(kind: ServiceManagerKind) -> &'static [&'static str] {
    variables_for_backend(kind)
}

pub(super) fn validate_persisted_bus_address(
    kind: ServiceManagerKind,
    address: Option<OsString>,
) -> Result<()> {
    if !import_variables(kind).contains(&"DBUS_SESSION_BUS_ADDRESS") {
        return Ok(());
    }
    let Some(address) = address else {
        return Ok(());
    };
    let address = address
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("session bus address is not valid UTF-8"))?;
    // Repair commands use the same stable-bus rule as fresh installations
    validate_session_bus_address(address, rustix::process::getuid().as_raw()).map_err(Into::into)
}

pub(super) fn validate_session_environment(
    get_var: impl FnMut(&str) -> Option<OsString>,
) -> Result<()> {
    let missing = missing_session_variables(get_var);
    // Both values identify the compositor session and its private runtime root
    if !missing.is_empty() {
        bail!(
            "missing session variables: {}; run from the compositor session",
            missing.join(", ")
        );
    }
    Ok(())
}

pub(super) fn missing_session_variables(
    mut get_var: impl FnMut(&str) -> Option<OsString>,
) -> Vec<&'static str> {
    // Empty variables are equivalent to absent variables for process launches
    ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"]
        .into_iter()
        .filter(|name| get_var(name).is_none_or(|value| value.is_empty()))
        .collect()
}
