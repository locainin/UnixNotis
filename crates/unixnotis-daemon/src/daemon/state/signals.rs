use unixnotis_core::{CloseReason, ControlState, PopupGateState, CONTROL_OBJECT_PATH};
use zbus::SignalContext;

use crate::daemon::{ControlServer, NotificationServer, NOTIFICATIONS_OBJECT_PATH};
use crate::store::NotificationStore;

use super::cache::should_emit_cached;
use super::DaemonState;

impl DaemonState {
    // Sends all the "this notification closed" messages that different listeners expect
    pub(in crate::daemon) async fn emit_close_fanout(
        &self,
        id: u32,
        reason: CloseReason,
    ) -> zbus::Result<()> {
        // Keep the first thing that goes wrong, but still try to send every signal
        let mut first_error = None;

        // Tell the standard notification interface that this notification closed
        self.emit_freedesktop_close(id, reason as u32, &mut first_error)
            .await;

        // Tell this daemon's control interface that the same notification closed
        self.emit_control_close(id, reason, &mut first_error).await;

        // After a close, the stored state may look different, so tell clients about that too
        if let Err(err) = self.emit_state_changed().await {
            record_signal_error(&mut first_error, err);
        }

        // Return success only if every attempted signal avoided errors
        first_error.map_or(Ok(()), Err)
    }

    // Sends the signals needed when a notification is dismissed by the user
    pub(in crate::daemon) async fn emit_dismiss_fanout(
        &self,
        id: u32,
        removed_active: bool,
    ) -> zbus::Result<()> {
        // Save the first error, while still giving the other signals a chance to run
        let mut first_error = None;

        // Only the active notification needs the freedesktop close signal here
        if removed_active {
            self.emit_freedesktop_close(id, CloseReason::DismissedByUser as u32, &mut first_error)
                .await;
        }

        // The control side is always told that the notification was dismissed
        self.emit_control_close(id, CloseReason::DismissedByUser, &mut first_error)
            .await;

        // Let clients know the visible daemon state may have changed after dismissal
        if let Err(err) = self.emit_state_changed().await {
            record_signal_error(&mut first_error, err);
        }

        // Give back the first error if any signal failed
        first_error.map_or(Ok(()), Err)
    }

    // Sends the close signal on the standard desktop notifications interface
    async fn emit_freedesktop_close(
        &self,
        id: u32,
        reason: u32,
        first_error: &mut Option<zbus::Error>,
    ) {
        // Build the D-Bus signal context for the normal notifications object path
        match SignalContext::new(&self.connection, NOTIFICATIONS_OBJECT_PATH) {
            Ok(notif_ctx) => {
                // Send the actual "notification closed" signal to desktop clients
                if let Err(err) =
                    NotificationServer::notification_closed(&notif_ctx, id, reason).await
                {
                    // Remember this error only if no earlier signal already failed
                    record_signal_error(first_error, err);
                }
            }
            // If the signal context cannot be made, remember that as the signal error
            Err(err) => record_signal_error(first_error, err),
        }
    }

    // Sends the close signal on this daemon's control interface
    async fn emit_control_close(
        &self,
        id: u32,
        reason: CloseReason,
        first_error: &mut Option<zbus::Error>,
    ) {
        // Build the D-Bus signal context for the control object path
        match SignalContext::new(&self.connection, CONTROL_OBJECT_PATH) {
            Ok(control_ctx) => {
                // Send the control-layer close event with the richer CloseReason enum
                if let Err(err) = ControlServer::notification_closed(&control_ctx, id, reason).await
                {
                    // Store the first failure so callers can still hear about a problem
                    record_signal_error(first_error, err);
                }
            }
            // If the control signal context fails, treat it like any other signal failure
            Err(err) => record_signal_error(first_error, err),
        }
    }

    // Rebuilds the current public state and tells clients only if something changed
    pub(in crate::daemon) async fn emit_state_changed(&self) -> zbus::Result<()> {
        // Lock the store briefly so we can take a clean snapshot of the current state
        let state = {
            let store = self.store.lock().await;
            control_state_from_store(&store)
        };

        // Work out whether popups should currently be allowed from that state
        let popup_gate = popup_gate_from_state(&state);

        // Duplicate broadcasts add D-Bus churn without changing UI behavior
        let should_emit_state = should_emit_cached(&self.last_emitted_state, &state);

        // Avoid sending the popup gate signal if clients already know this value
        let should_emit_popup_gate = should_emit_cached(&self.last_emitted_popup_gate, &popup_gate);

        // If neither value changed, there is nothing useful to send
        if !should_emit_any_state_signal(should_emit_state, should_emit_popup_gate) {
            return Ok(());
        }

        // Create one control context and reuse it for whichever state signals are needed
        let control_ctx = SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)?;

        // Keep the first send error while still trying the other state signal
        let mut first_error = None;

        // Send the full state update only when the cached state says it is new
        if should_emit_state {
            if let Err(err) = ControlServer::state_changed(&control_ctx, state).await {
                record_signal_error(&mut first_error, err);
            }
        }

        // Send the popup gate update only when that specific value changed
        if should_emit_popup_gate {
            if let Err(err) = ControlServer::popup_gate_changed(&control_ctx, popup_gate).await {
                record_signal_error(&mut first_error, err);
            }
        }

        // Report the first signal error, or success if both needed signals worked
        first_error.map_or(Ok(()), Err)
    }

    // Tells clients to throw away their cached snapshot and fetch a fresh one
    pub async fn emit_snapshot_invalidated(&self) -> zbus::Result<()> {
        // This signal tells clients their local materialized view may be stale
        let control_ctx = SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)?;
        ControlServer::snapshot_invalidated(&control_ctx).await
    }
}

pub(in crate::daemon::state) fn control_state_from_store(
    store: &NotificationStore,
) -> ControlState {
    // Panel consumers still need history and inhibitor counters in one snapshot
    ControlState {
        dnd_enabled: store.dnd_enabled(),
        history_count: store.history_len() as u32,
        inhibited: store.inhibited(),
        inhibitor_count: store.inhibitor_count(),
    }
}

pub(in crate::daemon::state) const fn popup_gate_from_state(
    state: &ControlState,
) -> PopupGateState {
    // Popup policy only depends on the gate, so history churn should not wake it up
    PopupGateState {
        dnd_enabled: state.dnd_enabled,
        inhibited: state.inhibited,
    }
}

pub(in crate::daemon::state) const fn should_emit_any_state_signal(
    should_emit_state: bool,
    should_emit_popup_gate: bool,
) -> bool {
    should_emit_state || should_emit_popup_gate
}

pub(in crate::daemon::state) fn record_signal_error(
    first_error: &mut Option<zbus::Error>,
    err: zbus::Error,
) {
    if first_error.is_none() {
        *first_error = Some(err);
    }
}
