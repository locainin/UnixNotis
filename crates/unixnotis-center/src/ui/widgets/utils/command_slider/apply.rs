//! Slider refresh output application

use std::time::{Duration, Instant};

use gtk::prelude::*;
use unixnotis_core::{util, PanelDebugLevel};

use super::super::slider_parse::{format_value, parse_muted, parse_numeric};
use super::request::SliderRefreshRequest;
use super::state::SliderRefreshState;
use super::value::slider_value_changed;
use crate::debug;
use crate::ui::perf_probe;

pub(super) fn apply_successful_output(
    request: &SliderRefreshRequest,
    refresh: &SliderRefreshState,
    stdout: &[u8],
    base_interval: Duration,
) {
    let stdout = String::from_utf8_lossy(stdout);
    let Some(value) = parse_numeric(&stdout, request.min, request.max, request.parse_mode) else {
        let snippet = util::log_snippet(stdout.trim());
        debug::log(PanelDebugLevel::Warn, || {
            format!(
                "slider parse failed cmd=\"{}\" output=\"{}\"",
                request.cmd, snippet
            )
        });
        note_slider_error(refresh, base_interval);
        return;
    };

    let muted = parse_muted(&stdout);
    let value_changed = apply_slider_value(request, refresh, value);
    let icon_changed = apply_slider_icon(refresh, muted);
    let changed = value_changed || icon_changed;
    refresh
        .backoff
        .borrow_mut()
        .note_success(Instant::now(), base_interval, changed);

    if changed {
        debug::log(PanelDebugLevel::Verbose, || {
            format!(
                "slider updated cmd=\"{}\" value={value:.1} muted={muted}",
                request.cmd
            )
        });
    }
}

pub(super) fn note_slider_error(refresh: &SliderRefreshState, base_interval: Duration) {
    refresh
        .backoff
        .borrow_mut()
        .note_error(Instant::now(), base_interval);
}

fn apply_slider_value(
    request: &SliderRefreshRequest,
    refresh: &SliderRefreshState,
    value: f64,
) -> bool {
    let formatted = format_value(value);
    // Skip widget writes when the visible state is already current
    let value_changed = slider_value_changed(refresh.scale.value(), value, request.step);
    let label_changed = refresh.label.text().as_str() != formatted;
    if !value_changed && !label_changed {
        return false;
    }

    refresh.updating.set(true);
    if value_changed {
        // Perf probes make flamegraph runs easier to line up with actual GTK writes
        perf_probe::slider_value_write();
        refresh.scale.set_value(value);
    }
    if label_changed {
        // Label writes are cheap, but avoiding them removes needless GTK invalidations
        perf_probe::slider_label_write();
        refresh.label.set_text(&formatted);
    }
    refresh.updating.set(false);
    true
}

fn apply_slider_icon(refresh: &SliderRefreshState, muted: bool) -> bool {
    let Some(icon_muted) = refresh.icon_muted.as_ref() else {
        return false;
    };

    // Not every slider has a muted icon pair
    let icon = if muted {
        icon_muted
    } else {
        &refresh.icon_name
    };

    // Slider refreshes can be frequent, so skip icon churn when nothing changed
    if refresh.icon_image.icon_name().as_deref() == Some(icon.as_str()) {
        return false;
    }

    perf_probe::slider_icon_write();
    refresh.icon_image.set_icon_name(Some(icon));
    true
}
