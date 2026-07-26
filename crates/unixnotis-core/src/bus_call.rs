//! Shared hard timeout for internal D-Bus method calls

use std::future::Future;
use std::time::Duration;

/// Maximum wait for one internal `UnixNotis` D-Bus method
pub const INTERNAL_DBUS_CALL_TIMEOUT: Duration = Duration::from_secs(2);

/// Run one D-Bus method with the internal hard timeout
///
/// # Errors
///
/// Returns the method error or a timeout error when the call exceeds the limit
pub async fn timed_dbus_call<T>(call: impl Future<Output = zbus::Result<T>>) -> zbus::Result<T> {
    timed_dbus_call_with_timeout(INTERNAL_DBUS_CALL_TIMEOUT, call).await
}

async fn timed_dbus_call_with_timeout<T>(
    timeout: Duration,
    call: impl Future<Output = zbus::Result<T>>,
) -> zbus::Result<T> {
    match tokio::time::timeout(timeout, call).await {
        Ok(result) => result,
        Err(_) => Err(zbus::Error::Failure(format!(
            "UnixNotis D-Bus call timed out after {} seconds",
            timeout.as_secs_f64()
        ))),
    }
}

#[cfg(test)]
#[path = "tests/bus_call.rs"]
mod tests;
