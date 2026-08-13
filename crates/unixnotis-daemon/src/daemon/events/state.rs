//! Control-state snapshots, popup gates, and cache invalidation signals

use unixnotis_core::{ControlState, PopupGateState};

use crate::daemon::{ControlServer, DaemonState};

use super::publisher::{record_first_error, DaemonEventPublisher};

impl DaemonState {
    pub(in crate::daemon) async fn publish_state_changed(&self) -> zbus::Result<()> {
        let _publication = self.events.ordered_publication().await;
        let state = {
            // Capture after ordering so delayed callers always observe the newest revision
            let store = self.store.lock().await;
            store.control_state()
        };
        self.events.state_changed(state).await
    }

    pub(in crate::daemon) async fn publish_snapshot_invalidated(&self) -> zbus::Result<()> {
        self.events.snapshot_invalidated().await
    }
}

impl DaemonEventPublisher {
    pub(super) async fn state_changed(&self, state: ControlState) -> zbus::Result<()> {
        let popup_gate = popup_gate_from_state(&state);
        let publish_state = should_publish_cached(&self.last_state, &state);
        let publish_popup_gate = should_publish_cached(&self.last_popup_gate, &popup_gate);
        if !should_publish_any_state_signal(publish_state, publish_popup_gate) {
            return Ok(());
        }

        // One context serves both related signals from the same captured state
        let context = self.control_context()?;
        let mut first_error = None;
        if publish_state {
            match ControlServer::state_changed(&context, state.clone()).await {
                Ok(()) => update_cached(&self.last_state, state),
                Err(error) => record_first_error(&mut first_error, error),
            }
        }
        if publish_popup_gate {
            match ControlServer::popup_gate_changed(&context, popup_gate.clone()).await {
                Ok(()) => update_cached(&self.last_popup_gate, popup_gate),
                Err(error) => record_first_error(&mut first_error, error),
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(super) async fn snapshot_invalidated(&self) -> zbus::Result<()> {
        let context = self.control_context()?;
        ControlServer::snapshot_invalidated(&context).await
    }
}

pub(super) fn should_publish_cached<T: PartialEq>(
    cache: &std::sync::Mutex<Option<T>>,
    next: &T,
) -> bool {
    // Poison recovery preserves availability after a prior panicking task
    let cached = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cached.as_ref() != Some(next)
}

pub(super) fn update_cached<T>(cache: &std::sync::Mutex<Option<T>>, published: T) {
    // A cache entry means the corresponding D-Bus send completed successfully
    let mut cached = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *cached = Some(published);
}

pub(super) const fn popup_gate_from_state(state: &ControlState) -> PopupGateState {
    PopupGateState {
        dnd_enabled: state.dnd_enabled,
        inhibited: state.inhibited,
    }
}

pub(super) const fn should_publish_any_state_signal(
    publish_state: bool,
    publish_popup_gate: bool,
) -> bool {
    publish_state || publish_popup_gate
}
