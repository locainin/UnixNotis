//! Shared raw Notify message fixture

use std::collections::HashMap;

use zbus::zvariant::OwnedValue;
use zbus::Message;

pub(super) fn notify_message(
    app_name: &str,
    app_icon: &str,
    summary: &str,
    body: &str,
    actions: Vec<String>,
    hints: HashMap<String, OwnedValue>,
) -> Message {
    Message::method("/org/freedesktop/Notifications", "Notify")
        .expect("method builder")
        .interface("org.freedesktop.Notifications")
        .expect("notification interface")
        .build(&(
            app_name, 0_u32, app_icon, summary, body, actions, hints, 0_i32,
        ))
        .expect("Notify message")
}
