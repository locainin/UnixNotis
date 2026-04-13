//! Slider set-command debounce and dispatch

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::super::run_command;
use super::value::format_command_value;
use unixnotis_core::PanelDebugLevel;

use crate::debug;

pub(super) fn schedule_command(
    pending: Rc<RefCell<Option<glib::SourceId>>>,
    pending_value: Rc<Cell<Option<f64>>>,
    cmd_template: String,
    value: f64,
    step: f64,
) {
    // Latest value wins while debounce timer is active
    pending_value.set(Some(value));
    if pending.borrow().is_some() {
        return;
    }

    let value_text = format_command_value(value, step);
    debug::log(PanelDebugLevel::Verbose, || {
        format!("slider set scheduled value={value_text}")
    });
    let pending_guard = pending.clone();
    let pending_value = pending_value.clone();
    let id = glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
        // Drain pending state and execute the most recent queued command
        let value = pending_value.replace(None);
        let _ = pending_guard.borrow_mut().take();
        if let Some(value) = value {
            let formatted = cmd_template.replace("{value}", &format_command_value(value, step));
            run_command(&formatted);
        }
        glib::ControlFlow::Break
    });
    *pending.borrow_mut() = Some(id);
}
