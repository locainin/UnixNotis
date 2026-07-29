//! Generation-bound popup visibility reporting

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::NotificationKey;

use super::try_send_command;
use crate::dbus::UiCommand;

#[derive(Clone)]
pub(in crate::ui) struct PopupVisibilityBinding {
    key: Rc<Cell<NotificationKey>>,
    reported: Rc<Cell<Option<NotificationKey>>>,
}

impl PopupVisibilityBinding {
    pub(in crate::ui) fn new(key: NotificationKey) -> Self {
        Self {
            key: Rc::new(Cell::new(key)),
            reported: Rc::new(Cell::new(None)),
        }
    }

    pub(in crate::ui) fn bind_generation(&self, key: NotificationKey) {
        if self.key.get() == key {
            return;
        }
        // A same-ID replacement needs its own visibility acknowledgement
        self.key.set(key);
        self.reported.set(None);
    }

    pub(in crate::ui) fn report_if_visible(
        &self,
        revealer: &gtk::Revealer,
        window: &gtk::ApplicationWindow,
        command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
    ) {
        let key = self.key.get();
        if self.reported.get() == Some(key) || !window.is_mapped() || !revealer.is_child_revealed()
        {
            return;
        }
        self.reported.set(Some(key));
        try_send_command(command_tx, UiCommand::Visible(key));
    }
}
