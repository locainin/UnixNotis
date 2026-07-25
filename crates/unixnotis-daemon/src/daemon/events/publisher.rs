//! Shared connection state and error policy for daemon event publication

use std::sync::Mutex as StdMutex;

use tokio::sync::{Mutex, MutexGuard};
use unixnotis_core::{ControlState, PopupGateState, CONTROL_OBJECT_PATH};
use zbus::{Connection, SignalContext};

use crate::daemon::NOTIFICATIONS_OBJECT_PATH;

pub(in crate::daemon) struct DaemonEventPublisher {
    connection: Connection,
    // One async guard keeps state-bearing signals in capture order across await points
    publication_order: Mutex<()>,
    // State snapshots are cached here because publication owns duplicate suppression
    pub(super) last_state: StdMutex<Option<ControlState>>,
    pub(super) last_popup_gate: StdMutex<Option<PopupGateState>>,
}

impl DaemonEventPublisher {
    pub(in crate::daemon) const fn new(connection: Connection) -> Self {
        Self {
            connection,
            publication_order: Mutex::const_new(()),
            last_state: StdMutex::new(None),
            last_popup_gate: StdMutex::new(None),
        }
    }

    pub(super) async fn ordered_publication(&self) -> MutexGuard<'_, ()> {
        // The guard spans store capture, signal fanout, and cache acknowledgement
        self.publication_order.lock().await
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
