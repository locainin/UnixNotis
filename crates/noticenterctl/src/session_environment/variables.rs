//! Session variables shared by service-manager backends

use std::ffi::OsString;

use anyhow::{bail, Result};

pub(super) const IMPORT_VARS: [&str; 8] = [
    "WAYLAND_DISPLAY",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "XDG_SESSION_DESKTOP",
    "DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "PATH",
];

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
