//! Shared connection state and error policy for daemon event publication

use std::sync::Mutex;

use unixnotis_core::{ControlState, PopupGateState, CONTROL_OBJECT_PATH};
use zbus::{Connection, SignalContext};

use crate::daemon::NOTIFICATIONS_OBJECT_PATH;

pub(in crate::daemon) struct DaemonEventPublisher {
    connection: Connection,
    // State snapshots are cached here because publication owns duplicate suppression
    pub(super) last_state: Mutex<Option<ControlState>>,
    pub(super) last_popup_gate: Mutex<Option<PopupGateState>>,
}

impl DaemonEventPublisher {
    pub(in crate::daemon) const fn new(connection: Connection) -> Self {
        Self {
            connection,
            last_state: Mutex::new(None),
            last_popup_gate: Mutex::new(None),
        }
    }

    pub(super) fn control_context(&self) -> zbus::Result<SignalContext<'_>> {
        SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)
    }

    pub(super) fn notification_context(&self) -> zbus::Result<SignalContext<'_>> {
        SignalContext::new(&self.connection, NOTIFICATIONS_OBJECT_PATH)
    }
}

pub(super) fn record_first_error(first_error: &mut Option<zbus::Error>, error: zbus::Error) {
    if first_error.is_none() {
        *first_error = Some(error);
    }
}
