use std::cell::RefCell;

use unixnotis_core::{InhibitorInfo, NotificationView, PanelDebugLevel};

use super::super::client::{ControlClient, ControlFuture};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RecordedCall {
    TogglePanel,
    OpenPanel,
    OpenPanelDebug(PanelDebugLevel),
    ClosePanel,
    ClearAll,
    ClearActive,
    ClearHistory,
    Dismiss(u32),
    ListActive,
    ListHistory,
    SetDnd(bool),
    SetDndUntil(i64),
    ToggleDnd,
    Inhibit { reason: String, scope: u32 },
    Uninhibit(u64),
    ListInhibitors,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RecordedEvent {
    Control(RecordedCall),
    DebugLogFollow,
}

#[derive(Default)]
pub(super) struct RecordingControlClient {
    events: RefCell<Vec<RecordedEvent>>,
}

impl RecordingControlClient {
    fn record<'a, T: 'a>(&'a self, call: RecordedCall, value: T) -> ControlFuture<'a, T> {
        Box::pin(async move {
            self.events.borrow_mut().push(RecordedEvent::Control(call));
            Ok(value)
        })
    }

    pub(super) fn record_debug_log_follow(&self) {
        self.events.borrow_mut().push(RecordedEvent::DebugLogFollow);
    }

    pub(super) fn take_events(&self) -> Vec<RecordedEvent> {
        self.events.replace(Vec::new())
    }

    pub(super) fn take_calls(&self) -> Vec<RecordedCall> {
        self.take_events()
            .into_iter()
            .filter_map(|event| match event {
                RecordedEvent::Control(call) => Some(call),
                RecordedEvent::DebugLogFollow => None,
            })
            .collect()
    }
}

impl ControlClient for RecordingControlClient {
    fn toggle_panel(&self) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::TogglePanel, ())
    }

    fn open_panel(&self) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::OpenPanel, ())
    }

    fn open_panel_debug(&self, level: PanelDebugLevel) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::OpenPanelDebug(level), ())
    }

    fn close_panel(&self) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::ClosePanel, ())
    }

    fn clear_all(&self) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::ClearAll, ())
    }

    fn clear_active(&self) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::ClearActive, ())
    }

    fn clear_history(&self) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::ClearHistory, ())
    }

    fn dismiss(&self, id: u32) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::Dismiss(id), ())
    }

    fn list_active(&self) -> ControlFuture<'_, Vec<NotificationView>> {
        self.record(RecordedCall::ListActive, Vec::new())
    }

    fn list_history(&self) -> ControlFuture<'_, Vec<NotificationView>> {
        self.record(RecordedCall::ListHistory, Vec::new())
    }

    fn set_dnd(&self, enabled: bool) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::SetDnd(enabled), ())
    }

    fn set_dnd_until(&self, expires_at: i64) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::SetDndUntil(expires_at), ())
    }

    fn toggle_dnd(&self) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::ToggleDnd, ())
    }

    fn inhibit<'a>(&'a self, reason: &'a str, scope: u32) -> ControlFuture<'a, u64> {
        Box::pin(async move {
            self.events
                .borrow_mut()
                .push(RecordedEvent::Control(RecordedCall::Inhibit {
                    reason: reason.to_owned(),
                    scope,
                }));
            Ok(42)
        })
    }

    fn uninhibit(&self, id: u64) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::Uninhibit(id), ())
    }

    fn list_inhibitors(&self) -> ControlFuture<'_, Vec<InhibitorInfo>> {
        self.record(RecordedCall::ListInhibitors, Vec::new())
    }
}
