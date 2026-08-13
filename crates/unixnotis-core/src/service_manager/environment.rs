//! Backend-specific session environment policy

use std::fmt;

use super::ServiceManagerKind;

const GRAPHICAL_SESSION_VARIABLES: [&str; 6] = [
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "XDG_RUNTIME_DIR",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "XDG_SESSION_DESKTOP",
];

const DIRECT_MANAGER_VARIABLES: [&str; 7] = [
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "XDG_RUNTIME_DIR",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "XDG_SESSION_DESKTOP",
    "DBUS_SESSION_BUS_ADDRESS",
];

/// Return the narrow environment allowlist for one service manager
#[must_use]
pub const fn variables_for_backend(kind: ServiceManagerKind) -> &'static [&'static str] {
    match kind {
        // systemd resolves the stable user bus through its own user-manager context
        ServiceManagerKind::Systemd => &GRAPHICAL_SESSION_VARIABLES,
        // Direct supervisors may need the stable user-bus address persisted explicitly
        ServiceManagerKind::Dinit | ServiceManagerKind::Runit | ServiceManagerKind::S6 => {
            &DIRECT_MANAGER_VARIABLES
        }
    }
}

/// Error returned when an installer shell points at a transient or nonstandard bus
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SessionBusAddressError {
    address: String,
}

impl fmt::Display for SessionBusAddressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "refusing to persist nonstandard session bus address: {}",
            self.address
        )
    }
}

impl std::error::Error for SessionBusAddressError {}

/// Require the standard per-user bus before persisting an explicit address
///
/// # Errors
///
/// Returns an error when the address does not name `/run/user/<uid>/bus`
pub fn validate_session_bus_address(address: &str, uid: u32) -> Result<(), SessionBusAddressError> {
    let expected = format!("unix:path=/run/user/{uid}/bus");
    if address == expected {
        return Ok(());
    }
    Err(SessionBusAddressError {
        address: address.to_string(),
    })
}
