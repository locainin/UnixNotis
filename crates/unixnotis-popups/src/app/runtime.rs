//! GTK runtime helpers for popup startup

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use glib::MainContext;

use super::reload::{start_reload_timer, ReloadGate};
use crate::{dbus, ui};

// Handles one UI event and then advances the reload gate state machine
pub(super) fn handle_ui_event(
    ui: &Rc<RefCell<ui::UiState>>,
    reload_gate: &Arc<ReloadGate>,
    event_tx: &async_channel::Sender<dbus::UiEvent>,
    reload_timer: &Arc<Mutex<Option<glib::SourceId>>>,
    event: dbus::UiEvent,
) {
    let is_css_reload = matches!(&event, dbus::UiEvent::CssReload);
    let is_config_reload = matches!(&event, dbus::UiEvent::ConfigReload);

    // Complete the reload only after the handler finishes so watcher hits
    // during handler work still schedule a trailing reload
    ui.borrow_mut().handle_event(event);
    let needs_retry_timer = if is_css_reload {
        reload_gate.complete_css(event_tx)
    } else if is_config_reload {
        reload_gate.complete_config(event_tx)
    } else {
        false
    };

    // Free queue space can now be reused by any pending reload retry
    reload_gate.flush(event_tx);
    if needs_retry_timer || reload_gate.has_pending() {
        // GTK owns the timer source, so schedule it back onto the main context
        let reload_gate = Arc::clone(reload_gate);
        let event_tx = event_tx.clone();
        let reload_timer = Arc::clone(reload_timer);
        MainContext::default().invoke(move || {
            start_reload_timer(&reload_gate, &event_tx, &reload_timer);
        });
    }
}
