//! D-Bus name acquisition helpers for the daemon entry flow

use tracing::info;
use unixnotis_core::CONTROL_BUS_NAME;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::Connection;

pub async fn request_well_known_name(
    connection: &Connection,
    replace_existing: bool,
) -> zbus::Result<RequestNameReply> {
    let flags = if replace_existing {
        // Trial mode is allowed to take over so the original daemon can be restored later
        zbus::fdo::RequestNameFlags::ReplaceExisting | zbus::fdo::RequestNameFlags::AllowReplacement
    } else {
        // Avoid being replaceable in non-trial mode to prevent silent takeovers
        zbus::fdo::RequestNameFlags::DoNotQueue.into()
    };
    connection
        .request_name_with_flags("org.freedesktop.Notifications", flags)
        .await
}

pub async fn request_control_name(connection: &Connection) -> zbus::Result<RequestNameReply> {
    // Control commands should fail fast if another daemon already owns the interface
    let flags = RequestNameFlags::DoNotQueue;
    connection
        .request_name_with_flags(CONTROL_BUS_NAME, flags.into())
        .await
}

pub fn log_name_reply(reply: &RequestNameReply) {
    match reply {
        RequestNameReply::PrimaryOwner => {
            info!("acquired org.freedesktop.Notifications");
        }
        RequestNameReply::InQueue => {
            info!("queued for org.freedesktop.Notifications");
        }
        RequestNameReply::AlreadyOwner => {
            info!("already owns org.freedesktop.Notifications");
        }
        RequestNameReply::Exists => {
            info!("org.freedesktop.Notifications is already owned");
        }
    }
}
