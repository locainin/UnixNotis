//! Command slider signal wiring

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use unixnotis_core::{PanelDebugLevel, SliderWidgetConfig};

use super::super::run_action_command_with_completion;
use super::super::slider_parse::format_value;
use super::layout::build_icon_shell;
use super::refresh::request_refresh;
use super::request::SliderRefreshRequest;
use super::schedule::schedule_command;
use super::state::{build_refresh_state_from_weak, SliderRefreshMeta};
use crate::debug;

pub(super) fn attach_icon_action(
    root: &gtk::Box,
    icon_image: &gtk::Image,
    scale: &gtk::Scale,
    value_label: &gtk::Label,
    config: &SliderWidgetConfig,
    refresh_meta: &SliderRefreshMeta,
) {
    let Some(toggle_cmd) = config.toggle_cmd.as_ref() else {
        // Static sliders still use the same shell widget as clickable sliders
        // This keeps default template alignment stable between volume and brightness rows
        let icon_shell = build_icon_shell(icon_image, false);
        root.prepend(&icon_shell);
        return;
    };

    let icon_button = build_icon_shell(icon_image, true);
    root.prepend(&icon_button);

    // Capture only weak widgets so pending callbacks do not keep a closed panel alive
    let cmd = toggle_cmd.clone();
    let request = SliderRefreshRequest::from_config(config);
    let scale_weak = scale.downgrade();
    let label_weak = value_label.downgrade();
    let icon_weak = icon_image.downgrade();
    let refresh_meta = refresh_meta.clone();
    icon_button.connect_clicked(move |_| {
        let scale_weak = scale_weak.clone();
        let label_weak = label_weak.clone();
        let icon_weak = icon_weak.clone();
        let request = request.clone();
        let refresh_meta = refresh_meta.clone();
        run_action_command_with_completion(cmd.clone(), "slider toggle action", move |failed| {
            if failed {
                // Failed actions still need one refresh so UI snaps back to real state
                debug::log(PanelDebugLevel::Warn, || {
                    format!(
                        "slider toggle action failed; forcing refresh cmd=\"{}\"",
                        request.cmd
                    )
                });
            }

            // The widget may have been destroyed before the async action completed
            let Some(refresh) =
                build_refresh_state_from_weak(&scale_weak, &label_weak, &icon_weak, &refresh_meta)
            else {
                return;
            };
            request_refresh(request.clone(), refresh, Duration::from_secs(1), true);
        });
    });
}

pub(super) fn attach_scale_action(
    scale: &gtk::Scale,
    value_label: &gtk::Label,
    icon_image: &gtk::Image,
    config: &SliderWidgetConfig,
    refresh_meta: &SliderRefreshMeta,
) {
    // Debounce state coalesces slider drags into fewer set_cmd executions
    let pending = Rc::new(RefCell::new(None));
    let pending_value = Rc::new(Cell::new(None));
    let request = SliderRefreshRequest::from_config(config);
    let updating_guard = refresh_meta.updating.clone();
    let pending_guard = pending.clone();
    let pending_value_guard = pending_value.clone();
    let scale_weak = scale.downgrade();
    let label_weak = value_label.downgrade();
    let icon_weak = icon_image.downgrade();
    let label_clone = value_label.clone();
    let refresh_meta_for_set = refresh_meta.clone();
    let step = config.step;
    let set_cmd = config.set_cmd.clone();

    scale.connect_value_changed(move |scale| {
        // Skip callback body when value is being updated programmatically
        if updating_guard.get() {
            return;
        }

        let value = scale.value();
        // Local label echo keeps dragging responsive before the debounced command finishes
        label_clone.set_text(&format_value(value));
        schedule_command(
            pending_guard.clone(),
            pending_value_guard.clone(),
            set_cmd.clone(),
            value,
            step,
            Rc::new({
                let scale_weak = scale_weak.clone();
                let label_weak = label_weak.clone();
                let icon_weak = icon_weak.clone();
                let request = request.clone();
                let refresh_meta = refresh_meta_for_set.clone();
                move |failed| {
                    if !failed {
                        return;
                    }

                    // Failed set actions should reconcile quickly instead of waiting for polling
                    debug::log(PanelDebugLevel::Warn, || {
                        format!(
                            "slider set action failed; forcing refresh cmd=\"{}\"",
                            request.cmd
                        )
                    });
                    // Corrective refresh uses the same parser and backoff path as polling
                    let Some(refresh) = build_refresh_state_from_weak(
                        &scale_weak,
                        &label_weak,
                        &icon_weak,
                        &refresh_meta,
                    ) else {
                        return;
                    };
                    request_refresh(request.clone(), refresh, Duration::from_secs(1), true);
                }
            }),
        );
    });
}
