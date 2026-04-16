use std::sync::Arc;

use futures_util::stream::{self, StreamExt};
use tracing::warn;
use unixnotis_core::{CloseReason, CONTROL_OBJECT_PATH};
use zbus::SignalContext;

use super::super::{DaemonState, NotificationServer, NOTIFICATIONS_OBJECT_PATH};
use super::ControlServer;

// Keep clear-all signal fanout bounded to avoid a burst of tiny tasks
const CLEAR_ALL_CONCURRENCY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ClearAllSignalPlan {
    pub(super) emit_close_signals: bool,
    pub(super) emit_snapshot_invalidated: bool,
    pub(super) emit_state_changed: bool,
}

pub(super) fn clear_all_signal_plan(ids: &[u32]) -> ClearAllSignalPlan {
    ClearAllSignalPlan {
        // Only active rows need close fanout
        emit_close_signals: !ids.is_empty(),
        // Even an empty clear can be the only thing that fixes a stale client list
        emit_snapshot_invalidated: true,
        // Counters still need a refresh chance after the clear path
        emit_state_changed: true,
    }
}

pub(super) async fn emit_clear_all_signals(state: &Arc<DaemonState>, ids: Vec<u32>) {
    let signal_plan = clear_all_signal_plan(&ids);

    if !signal_plan.emit_close_signals {
        emit_post_clear_refresh(state, signal_plan).await;
        return;
    }

    let notif_ctx = SignalContext::new(state.connection(), NOTIFICATIONS_OBJECT_PATH).ok();
    let control_ctx = SignalContext::new(state.connection(), CONTROL_OBJECT_PATH).ok();
    if notif_ctx.is_none() || control_ctx.is_none() {
        // The clear already happened
        warn!("failed to build signal context for clear_all; continuing with local state");
    }

    // Emit close signals with a bounded concurrency limit to avoid task spikes
    stream::iter(ids)
        .for_each_concurrent(CLEAR_ALL_CONCURRENCY, move |id| {
            let notif_ctx = notif_ctx.clone();
            let control_ctx = control_ctx.clone();
            async move {
                if let Some(notif_ctx) = notif_ctx.as_ref() {
                    if let Err(err) = NotificationServer::notification_closed(
                        notif_ctx,
                        id,
                        CloseReason::DismissedByUser as u32,
                    )
                    .await
                    {
                        warn!(
                            ?err,
                            id, "failed to emit notification_closed during clear_all"
                        );
                    }
                }
                if let Some(control_ctx) = control_ctx.as_ref() {
                    if let Err(err) = ControlServer::notification_closed(
                        control_ctx,
                        id,
                        CloseReason::DismissedByUser,
                    )
                    .await
                    {
                        warn!(
                            ?err,
                            id, "failed to emit control notification_closed during clear_all"
                        );
                    }
                }
            }
        })
        .await;

    emit_post_clear_refresh(state, signal_plan).await;
}

async fn emit_post_clear_refresh(state: &Arc<DaemonState>, signal_plan: ClearAllSignalPlan) {
    if signal_plan.emit_snapshot_invalidated {
        if let Err(err) = state.emit_snapshot_invalidated().await {
            // Clients can still fall back to later reconnect seeding if this broadcast is missed
            warn!(?err, "failed to emit snapshot_invalidated after clear_all");
        }
    }
    if signal_plan.emit_state_changed {
        if let Err(err) = state.emit_state_changed().await {
            // State was updated locally even if listeners missed this broadcast
            warn!(?err, "failed to emit state_changed after clear_all");
        }
    }
}
