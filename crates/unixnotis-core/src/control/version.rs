//! Private control-interface version negotiation

use thiserror::Error;

use super::{ControlProxy, CONTROL_API_VERSION};
use crate::timed_dbus_call;

/// Failure to prove that `UnixNotis` components share one control contract
#[derive(Debug, Error)]
pub enum ControlApiVersionError {
    #[error("read UnixNotis control API version: {0}")]
    Transport(#[from] zbus::Error),
    #[error("UnixNotis component version mismatch: expected {expected}, got {actual}")]
    Mismatch { expected: u32, actual: u32 },
}

/// Require the daemon and client to use the same private interface version
///
/// # Errors
///
/// Returns a transport error when the version cannot be read or a mismatch error when the daemon
/// and client use different private control contracts
pub async fn ensure_control_api_version(
    proxy: &ControlProxy<'_>,
) -> Result<(), ControlApiVersionError> {
    let actual = timed_dbus_call(proxy.get_api_version()).await?;
    if actual == CONTROL_API_VERSION {
        Ok(())
    } else {
        Err(ControlApiVersionError::Mismatch {
            expected: CONTROL_API_VERSION,
            actual,
        })
    }
}
