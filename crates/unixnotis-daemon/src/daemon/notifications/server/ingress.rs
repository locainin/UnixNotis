//! Raw notification method guard applied before zbus deserializes owned payload fields

use std::collections::HashMap;
use std::fmt::Write;

use zbus::names::{InterfaceName, MemberName};
use zbus::object_server::{DispatchResult, Interface, SignalContext};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, Message, ObjectServer};

use super::notify_body::{preflight_notify, PreflightError};
use super::NotificationServer;

// This leaves room for one maximum image plus bounded text, actions, hints, and wire overhead
pub(super) const MAX_NOTIFY_WIRE_BODY_BYTES: usize = 384 * 1024;

/// Object-server adapter that rejects oversized Notify bodies before typed allocation
pub struct NotificationIngress {
    inner: NotificationServer,
}

impl NotificationIngress {
    pub const fn new(inner: NotificationServer) -> Self {
        Self { inner }
    }
}

#[zbus::export::async_trait::async_trait]
impl Interface for NotificationIngress {
    fn name() -> InterfaceName<'static> {
        <NotificationServer as Interface>::name()
    }

    fn spawn_tasks_for_methods(&self) -> bool {
        self.inner.spawn_tasks_for_methods()
    }

    async fn get(&self, property_name: &str) -> Option<zbus::fdo::Result<OwnedValue>> {
        self.inner.get(property_name).await
    }

    async fn get_all(&self) -> zbus::fdo::Result<HashMap<String, OwnedValue>> {
        self.inner.get_all().await
    }

    fn set<'call>(
        &'call self,
        property_name: &'call str,
        value: &'call Value<'_>,
        context: &'call SignalContext<'_>,
    ) -> DispatchResult<'call> {
        self.inner.set(property_name, value, context)
    }

    async fn set_mut(
        &mut self,
        property_name: &str,
        value: &Value<'_>,
        context: &SignalContext<'_>,
    ) -> Option<zbus::fdo::Result<()>> {
        self.inner.set_mut(property_name, value, context).await
    }

    fn call<'call>(
        &'call self,
        server: &'call ObjectServer,
        connection: &'call Connection,
        message: &'call Message,
        name: MemberName<'call>,
    ) -> DispatchResult<'call> {
        if notify_body_is_oversized(name.as_str(), message.body().len()) {
            // Construct the D-Bus error without asking the typed interface to decode the body
            return DispatchResult::new_async(connection, message, async {
                Err::<(), _>(zbus::fdo::Error::LimitsExceeded(format!(
                    "Notify body exceeds {MAX_NOTIFY_WIRE_BODY_BYTES} bytes"
                )))
            });
        }
        if name.as_bytes() == b"Notify" {
            if let Err(error) = preflight_notify(message) {
                // Structural limits are checked from borrowed bytes before owned argument decoding
                return DispatchResult::new_async(connection, message, async move {
                    match error {
                        PreflightError::LimitsExceeded(reason) => {
                            Err::<(), _>(zbus::fdo::Error::LimitsExceeded(reason.to_string()))
                        }
                        PreflightError::Malformed(reason) => {
                            Err::<(), _>(zbus::fdo::Error::InvalidArgs(reason.to_string()))
                        }
                    }
                });
            }
        }
        self.inner.call(server, connection, message, name)
    }

    fn call_mut<'call>(
        &'call mut self,
        server: &'call ObjectServer,
        connection: &'call Connection,
        message: &'call Message,
        name: MemberName<'call>,
    ) -> DispatchResult<'call> {
        // NotificationServer currently has no mutable methods, but delegation preserves its API
        self.inner.call_mut(server, connection, message, name)
    }

    fn introspect_to_writer(&self, writer: &mut dyn Write, level: usize) {
        self.inner.introspect_to_writer(writer, level);
    }
}

fn notify_body_is_oversized(member: &str, body_len: usize) -> bool {
    member.as_bytes() == b"Notify" && body_len > MAX_NOTIFY_WIRE_BODY_BYTES
}

#[cfg(test)]
#[path = "tests/ingress.rs"]
mod tests;
