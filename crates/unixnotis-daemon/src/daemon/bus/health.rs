//! Session-bus identity, ownership verification, and runtime health checks

use std::time::Duration;

use anyhow::{anyhow, ensure, Context, Result};
use tracing::warn;
use unixnotis_core::{CONTROL_BUS_NAME, NOTIFICATIONS_BUS_NAME};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::Connection;

const BUS_HEALTH_INTERVAL: Duration = Duration::from_secs(1);
const BUS_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_CONSECUTIVE_TRANSIENT_FAILURES: u8 = 3;

#[derive(Debug)]
enum BusProbeOutcome {
    Healthy,
    DefinitiveNameLoss {
        name: &'static str,
        owner: Option<String>,
    },
    DefinitiveTransportFailure(anyhow::Error),
    TransientFailure(anyhow::Error),
}

#[derive(Debug, Default)]
struct TransientFailureCounter {
    consecutive: u8,
}

impl TransientFailureCounter {
    const fn observe_healthy(&mut self) {
        self.consecutive = 0;
    }

    const fn observe_failure(&mut self) -> bool {
        self.consecutive = self.consecutive.saturating_add(1);
        self.consecutive >= MAX_CONSECUTIVE_TRANSIENT_FAILURES
    }
}

pub async fn verify_name_owner(
    dbus: &DBusProxy<'_>,
    connection: &Connection,
    name: &'static str,
) -> Result<()> {
    let expected = connection
        .unique_name()
        .context("session bus did not assign a unique name")?;
    let bus_name = BusName::try_from(name).context("invalid required D-Bus name")?;
    let actual = tokio::time::timeout(BUS_PROBE_TIMEOUT, dbus.get_name_owner(bus_name))
        .await
        .with_context(|| format!("D-Bus owner probe timed out for {name}"))?
        .with_context(|| format!("D-Bus owner probe failed for {name}"))?;

    ensure!(
        actual.as_str() == expected.as_str(),
        "{name} owner mismatch: expected {expected}, found {actual}"
    );
    Ok(())
}

pub async fn monitor_required_bus_names(connection: Connection) -> Result<()> {
    let dbus = DBusProxy::new(&connection)
        .await
        .context("create D-Bus health proxy")?;

    let expected = connection
        .unique_name()
        .context("session bus did not assign a unique name")?
        .to_string();
    let mut transient_failures = TransientFailureCounter::default();
    loop {
        tokio::time::sleep(BUS_HEALTH_INTERVAL).await;
        match probe_required_names(&dbus, &expected).await {
            BusProbeOutcome::Healthy => transient_failures.observe_healthy(),
            BusProbeOutcome::DefinitiveNameLoss { name, owner } => {
                anyhow::bail!("lost required D-Bus name {name}; owner={owner:?}");
            }
            BusProbeOutcome::DefinitiveTransportFailure(error) => {
                return Err(error).context("session bus connection is closed");
            }
            BusProbeOutcome::TransientFailure(error) => {
                let fatal = transient_failures.observe_failure();
                warn!(
                    ?error,
                    transient_failures = transient_failures.consecutive,
                    "transient D-Bus health probe failure"
                );
                if fatal {
                    return Err(error).context("repeated D-Bus health failures");
                }
            }
        }
    }
}

async fn probe_required_names(dbus: &DBusProxy<'_>, expected: &str) -> BusProbeOutcome {
    for required in [NOTIFICATIONS_BUS_NAME, CONTROL_BUS_NAME] {
        let bus_name =
            BusName::try_from(required).expect("static required D-Bus name must be valid");
        let reply = tokio::time::timeout(BUS_PROBE_TIMEOUT, dbus.get_name_owner(bus_name)).await;
        match reply {
            Ok(Ok(owner)) if owner.as_str() == expected => {}
            Ok(Ok(owner)) => {
                return BusProbeOutcome::DefinitiveNameLoss {
                    name: required,
                    owner: Some(owner.to_string()),
                };
            }
            Ok(Err(error)) => return probe_error_outcome(required, error),
            Err(error) => {
                return BusProbeOutcome::TransientFailure(anyhow!(
                    "D-Bus owner probe timed out for {required}: {error}"
                ));
            }
        }
    }
    BusProbeOutcome::Healthy
}

fn probe_error_outcome(name: &'static str, error: zbus::fdo::Error) -> BusProbeOutcome {
    if matches!(error, zbus::fdo::Error::NameHasNoOwner(_)) {
        return BusProbeOutcome::DefinitiveNameLoss { name, owner: None };
    }
    let message = anyhow!("D-Bus owner probe failed for {name}: {error}");
    if definitive_transport_failure(&error) {
        BusProbeOutcome::DefinitiveTransportFailure(message)
    } else {
        BusProbeOutcome::TransientFailure(message)
    }
}

fn definitive_transport_failure(error: &zbus::fdo::Error) -> bool {
    match error {
        zbus::fdo::Error::Disconnected(_) => true,
        zbus::fdo::Error::ZBus(zbus::Error::InputOutput(error)) => matches!(
            error.kind(),
            std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::NotConnected
                | std::io::ErrorKind::UnexpectedEof
        ),
        _ => false,
    }
}

#[cfg(test)]
#[path = "tests/health.rs"]
mod tests;
