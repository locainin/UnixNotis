//! Raw notification method guard applied before zbus deserializes owned payload fields

use std::collections::HashMap;
use std::fmt::Write;

use zbus::names::{InterfaceName, MemberName};
use zbus::object_server::{DispatchResult, Interface, SignalContext};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, Message, ObjectServer};

use super::notify_body::{preflight_notify, PreflightError, MAX_NOTIFY_WIRE_BODY_BYTES};
use super::reply_lifecycle::PostReplyKey;
use super::NotificationServer;

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
        if notify_has_unix_fds(name.as_str(), message.header().unix_fds()) {
            // Notify has no descriptor-bearing fields, so attached descriptors are always invalid
            return DispatchResult::new_async(connection, message, async {
                Err::<(), _>(zbus::fdo::Error::InvalidArgs(
                    "Notify does not accept Unix file descriptors".to_string(),
                ))
            });
        }
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
        let is_notify = name.as_bytes() == b"Notify";
        let dispatch = self.inner.call(server, connection, message, name);
        if !is_notify {
            return dispatch;
        }

        let request = PostReplyKey::from_header(&message.header());
        match dispatch {
            DispatchResult::Async(future) => DispatchResult::Async(Box::pin(async move {
                // The generated handler sends the method reply before this future completes
                let reply_result = future.await;
                let suppressed = self.inner.post_reply_lifecycle.take(&request).await;
                if reply_result.is_ok() {
                    if let Some(suppressed) = suppressed {
                        // The signal now enters the connection after the successful reply
                        self.inner.publish_suppressed_close(suppressed).await;
                    }
                }
                reply_result
            })),
            other => other,
        }
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

fn notify_has_unix_fds(member: &str, unix_fds: Option<u32>) -> bool {
    member.as_bytes() == b"Notify" && unix_fds.is_some_and(|count| count != 0)
}

#[cfg(test)]
#[path = "tests/ingress.rs"]
mod tests;
