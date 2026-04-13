//! Slider refresh state and async refresh execution

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gtk::prelude::*;
use tracing::warn;
use unixnotis_core::{util, NumericParseMode, PanelDebugLevel};

use super::super::run_command_capture_status_async;
use super::super::slider_parse::{format_value, parse_muted, parse_numeric};
use super::value::slider_value_changed;
use crate::debug;

#[derive(Clone)]
pub(super) struct SliderRefreshState {
    // Slider updated from command output
    pub(super) scale: gtk::Scale,
    // Label kept in sync with the slider
    pub(super) label: gtk::Label,
    // Icon button updated after refresh
    pub(super) icon_button: gtk::Button,
    // Guard stops refresh writes from triggering another set command
    pub(super) updating: Rc<Cell<bool>>,
    // Generation drops stale async refresh results
    pub(super) refresh_gen: Arc<AtomicU64>,
    // Normal icon shown when not muted
    pub(super) icon_name: String,
    // Optional icon used when muted
    pub(super) icon_muted: Option<String>,
}

#[derive(Clone)]
pub(super) struct SliderRefreshMeta {
    // Non-widget refresh state that is safe to hold across signal closures
    pub(super) updating: Rc<Cell<bool>>,
    // Generation drops stale async refresh results
    pub(super) refresh_gen: Arc<AtomicU64>,
    // Normal icon shown when not muted
    pub(super) icon_name: String,
    // Optional icon used when muted
    pub(super) icon_muted: Option<String>,
}

pub(super) fn refresh_inner(
    cmd: String,
    min: f64,
    max: f64,
    step: f64,
    parse_mode: NumericParseMode,
    refresh: SliderRefreshState,
) {
    // New refresh id makes older async results stale
    let gen = refresh.refresh_gen.fetch_add(1, Ordering::Relaxed) + 1;

    let rx = run_command_capture_status_async(&cmd);
    let refresh_gen = refresh.refresh_gen.clone();
    glib::MainContext::default().spawn_local(async move {
        let output = match rx.recv().await {
            Ok(output) => output,
            Err(_) => return,
        };
        if refresh_gen.load(Ordering::Relaxed) != gen {
            // A newer refresh already started so this result is old
            return;
        }
        let output = match output {
            Ok(output) => output,
            Err(err) => {
                warn!(?err, "slider command failed");
                return;
            }
        };
        if !output.status.success() {
            warn!(?cmd, "slider command returned error");
            return;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let value = match parse_numeric(&stdout, min, max, parse_mode) {
            Some(value) => value,
            None => {
                let snippet = util::log_snippet(stdout.trim());
                debug::log(PanelDebugLevel::Warn, || {
                    format!("slider parse failed cmd=\"{}\" output=\"{}\"", cmd, snippet)
                });
                return;
            }
        };
        let muted = parse_muted(&stdout);

        let formatted = format_value(value);
        // Skip widget writes when the visible state is already current
        let value_changed = slider_value_changed(refresh.scale.value(), value, step);
        let label_changed = refresh.label.text().as_str() != formatted;
        if value_changed || label_changed {
            refresh.updating.set(true);
            if value_changed {
                refresh.scale.set_value(value);
            }
            if label_changed {
                refresh.label.set_text(&formatted);
            }
            refresh.updating.set(false);
            debug::log(PanelDebugLevel::Verbose, || {
                format!(
                    "slider updated cmd=\"{}\" value={value:.1} muted={muted}",
                    cmd
                )
            });
        }
        if let Some(icon_muted) = refresh.icon_muted.as_ref() {
            // Not every slider has a muted icon pair
            let icon = if muted {
                icon_muted
            } else {
                &refresh.icon_name
            };
            refresh.icon_button.set_icon_name(icon);
        }
    });
}
