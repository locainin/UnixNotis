//! Exclusive daemon-activation reservation for the release switch boundary

use std::time::Duration;

use anyhow::{bail, Context, Result};
use zbus::fdo::{RequestNameFlags, RequestNameReply};

const RESERVATION_TIMEOUT: Duration = Duration::from_secs(2);

pub struct DaemonActivationReservation {
    backing: Box<dyn ReservationBacking>,
}

trait ReservationBacking {}

struct LiveReservationBacking {
    // Keep the connection before the runtime so the names are released while
    // the runtime that owns the connection is still alive
    _connection: zbus::Connection,
    _runtime: tokio::runtime::Runtime,
}

impl ReservationBacking for LiveReservationBacking {}

impl DaemonActivationReservation {
    pub fn acquire() -> Result<Self> {
        Self::acquire_names(&[
            unixnotis_core::NOTIFICATIONS_BUS_NAME,
            unixnotis_core::CONTROL_BUS_NAME,
        ])
    }

    fn acquire_names(names: &[&str]) -> Result<Self> {
        if names.is_empty() {
            bail!("daemon activation reservation requires at least one D-Bus name")
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("create daemon-activation reservation runtime")?;
        let address = format!(
            "unix:path=/run/user/{}/bus",
            rustix::process::getuid().as_raw()
        );
        let connection = runtime.block_on(async {
            let builder = zbus::connection::Builder::address(address.as_str())
                .context("prepare stable user-bus reservation connection")?;
            let connection = tokio::time::timeout(RESERVATION_TIMEOUT, builder.build())
                .await
                .context("daemon-activation reservation connection timed out")?
                .context("connect to stable user bus for daemon-activation reservation")?;
            for &name in names {
                let reply = match tokio::time::timeout(
                    RESERVATION_TIMEOUT,
                    connection.request_name_with_flags(name, RequestNameFlags::DoNotQueue.into()),
                )
                .await
                .with_context(|| format!("D-Bus activation reservation for {name} timed out"))?
                {
                    Ok(reply) => reply,
                    Err(zbus::Error::NameTaken) => {
                        bail!("D-Bus activation name {name} became owned before release activation")
                    }
                    Err(error) => {
                        return Err(error).with_context(|| {
                            format!("request D-Bus activation reservation for {name}")
                        })
                    }
                };
                match reply {
                    RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {}
                    RequestNameReply::InQueue | RequestNameReply::Exists => {
                        bail!("D-Bus activation name {name} became owned before release activation")
                    }
                }
            }
            // Dropping this one connection releases every name if a later request failed
            Ok(connection)
        })?;

        Ok(Self {
            backing: Box::new(LiveReservationBacking {
                _connection: connection,
                _runtime: runtime,
            }),
        })
    }
}

impl Drop for DaemonActivationReservation {
    fn drop(&mut self) {
        // Keep the capability backing explicit while its boxed owner performs
        // the normal connection-before-runtime drop sequence
        let _ = &self.backing;
    }
}

#[cfg(test)]
#[path = "tests/name_reservation.rs"]
mod tests;
