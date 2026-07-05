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
    ToggleDnd,
    Inhibit { reason: String, scope: u32 },
    Uninhibit(u64),
    ListInhibitors,
}

#[derive(Default)]
pub(super) struct RecordingControlClient {
    calls: RefCell<Vec<RecordedCall>>,
}

impl RecordingControlClient {
    fn record<'a, T: 'a>(&'a self, call: RecordedCall, value: T) -> ControlFuture<'a, T> {
        Box::pin(async move {
            self.calls.borrow_mut().push(call);
            Ok(value)
        })
    }

    pub(super) fn take_calls(&self) -> Vec<RecordedCall> {
        self.calls.replace(Vec::new())
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

    fn toggle_dnd(&self) -> ControlFuture<'_, ()> {
        self.record(RecordedCall::ToggleDnd, ())
    }

    fn inhibit<'a>(&'a self, reason: &'a str, scope: u32) -> ControlFuture<'a, u64> {
        Box::pin(async move {
            self.calls.borrow_mut().push(RecordedCall::Inhibit {
                reason: reason.to_owned(),
                scope,
            });
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
