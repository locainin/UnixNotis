use std::future::Future;
use std::time::Duration;

use anyhow::{anyhow, Result};

const CONTROL_CALL_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) async fn run_control_call<T>(call: impl Future<Output = zbus::Result<T>>) -> Result<T> {
    // Race the real D-Bus call against the maximum allowed wait time
    match tokio::time::timeout(CONTROL_CALL_TIMEOUT, call).await {
        // The daemon answered in time and the call itself worked
        Ok(Ok(value)) => Ok(value),

        // The daemon answered in time, but reported a D-Bus error
        Ok(Err(err)) => Err(err.into()),

        // The daemon did not answer before the timeout finished
        Err(_) => Err(anyhow!(
            "timed out waiting for unixnotis daemon response after {}s",
            CONTROL_CALL_TIMEOUT.as_secs()
        )),
    }
}
