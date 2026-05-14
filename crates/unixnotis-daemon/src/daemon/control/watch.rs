//! Name-owner watch for automatic inhibitor cleanup
//!
//! When a controlling client exits, its inhibitors should not remain forever

use std::sync::Arc;

use futures_util::StreamExt;
use tracing::warn;
use unixnotis_core::CONTROL_OBJECT_PATH;
use zbus::fdo::DBusProxy;
use zbus::SignalContext;

use crate::daemon::{ControlServer, DaemonState};

pub(super) async fn spawn_inhibitor_owner_watch(state: Arc<DaemonState>) -> zbus::Result<()> {
    // Subscribe once and process updates in the background
    let proxy = DBusProxy::new(state.connection()).await?;
    let mut stream = proxy.receive_name_owner_changed().await?;

    tokio::spawn(async move {
        while let Some(signal) = stream.next().await {
            let args = match signal.args() {
                Ok(args) => args,
                Err(err) => {
                    warn!(?err, "failed to decode NameOwnerChanged args");
                    continue;
                }
            };

            // Ignore owner-acquired events and only process owner-lost events
            if args.new_owner().is_some() {
                continue;
            }
            let owner = args.name().to_string();

            // Remove inhibitors owned by the disconnected bus name
            let (changed, active, count) = {
                let mut store = state.store.lock().await;
                let changed = store.remove_inhibitors_by_owner(&owner);
                let active = store.inhibited();
                let count = store.inhibitor_count();
                (changed, active, count)
            };
            if !changed {
                continue;
            }

            // Build signal context each time so failure never blocks store cleanup
            let ctx = match SignalContext::new(state.connection(), CONTROL_OBJECT_PATH) {
                Ok(ctx) => ctx,
                Err(err) => {
                    warn!(?err, "failed to build signal context for inhibitor cleanup");
                    continue;
                }
            };

            // Notify listeners so UI can refresh inhibition badges immediately
            if let Err(err) = ControlServer::inhibitors_changed(&ctx, active, count).await {
                warn!(
                    ?err,
                    "failed to emit inhibitors_changed after owner disconnect"
                );
            }
            if let Err(err) = state.emit_state_changed().await {
                warn!(?err, "failed to emit state_changed after owner disconnect");
            }
        }
    });

    Ok(())
}
