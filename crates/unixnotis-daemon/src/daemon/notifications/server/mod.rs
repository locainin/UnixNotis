//! Freedesktop notification D-Bus server and request handling

mod avatar;
mod capabilities;
mod close;
mod flow;
mod ingress;
mod interface;
mod notify_body;
mod reply_lifecycle;
mod wire_hints;

pub use ingress::NotificationIngress;
pub use interface::NotificationServer;

use super::identity::SenderMetadata;
use super::ingress::quota::QuotaPrincipal;

fn quota_principal(sender: &SenderMetadata) -> Option<QuotaPrincipal> {
    Some(QuotaPrincipal::new(
        sender.sender_uid?,
        sender.sender_pid?,
        sender.sender_start_time?,
    ))
}

#[cfg(test)]
#[path = "tests/interface.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/quota_principal.rs"]
mod quota_principal_tests;
