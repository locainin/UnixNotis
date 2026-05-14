//! Card-style widgets for summary content

mod build;
mod calendar;
mod common;
#[cfg(test)]
#[path = "tests/cards.rs"]
mod tests;
mod weather;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::glib;
use gtk::prelude::*;
use tracing::warn;
use unixnotis_core::{CardWidgetConfig, PanelDebugLevel, WidgetPluginConfig};

use self::common::apply_cached_value;
use super::plugin::{parse_card_plugin_payload, PluginOutputLimits};
use super::utils::{
    run_command_capture_async, run_command_capture_with_timeout_async, RefreshBackoff,
};
use crate::debug;

pub struct CardGrid {
    // FlowBox root is embedded directly by the panel widget layout
    root: gtk::FlowBox,
    // Item list is retained for refresh cadence and due-time aggregation
    items: Vec<CardItem>,
}

struct CardItem {
    // Raw config is retained for command and plugin refresh decisions
    config: CardWidgetConfig,
    // Root card container inserted into the grid
    root: gtk::Box,
    // Title line shown in the card header
    title_label: gtk::Label,
    // Body label used by non-calendar cards
    body_label: gtk::Label,
    // Optional calendar widget for calendar-type cards
    calendar: Option<gtk::Calendar>,
    // Fast branch for calendar-specific refresh behavior
    is_calendar: bool,
    // Guard blocks overlapping async refresh calls
    inflight: Rc<Cell<bool>>,
    // Cached payload avoids visual churn when output is unchanged
    last_value: Rc<RefCell<Option<String>>>,
    // Backoff reduces repeated command executions when the value is stable
    refresh_backoff: Rc<RefCell<RefreshBackoff>>,
    // Calendar only changes daily; track the last rendered day to avoid redundant updates
    last_calendar_day: Rc<Cell<Option<(i32, i32, i32)>>>,
    // Schedules the next calendar update directly at the next local midnight
    calendar_next_due: Rc<Cell<Option<Instant>>>,
}

impl CardItem {
    fn refresh(&self, base_interval: Duration, force: bool) {
        if self.is_calendar {
            debug::log(PanelDebugLevel::Verbose, || "calendar refresh".to_string());
            let now = Instant::now();
            if !force {
                // Calendar content only changes at day boundaries, so skip work until midnight
                if let Some(next_due) = self.calendar_next_due.get() {
                    if now < next_due {
                        return;
                    }
                }
            }
            self.refresh_calendar(base_interval);
            return;
        }
        if !self.root.is_visible() {
            return;
        }
        let now = Instant::now();
        if !self.refresh_backoff.borrow().should_refresh(now, force) {
            return;
        }
        debug::log(PanelDebugLevel::Verbose, || {
            format!("card refresh: {}", self.config.title)
        });
        if self.inflight.get() {
            return;
        }
        if let Some(plugin) = self.config.plugin.as_ref() {
            self.refresh_plugin(plugin, base_interval);
            return;
        }
        let Some(cmd) = self.config.cmd.as_ref() else {
            self.refresh_backoff
                .borrow_mut()
                .note_success(Instant::now(), base_interval, false);
            return;
        };
        self.inflight.set(true);
        let cmd = cmd.clone();
        let rx = run_command_capture_async(&cmd);
        let label = self.body_label.clone();
        let inflight = self.inflight.clone();
        let last_value = self.last_value.clone();
        let refresh_backoff = self.refresh_backoff.clone();
        glib::MainContext::default().spawn_local(async move {
            let output = match rx.recv().await {
                Ok(output) => output,
                Err(_) => {
                    inflight.set(false);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            inflight.set(false);
            let output = match output {
                Ok(output) => output,
                Err(err) => {
                    warn!(?cmd, ?err, "info card command failed");
                    apply_cached_value(&label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            if !output.status.success() {
                warn!(?cmd, "info card command failed");
                apply_cached_value(&label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let value = stdout.trim();
            if value.is_empty() {
                apply_cached_value(&label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_success(Instant::now(), base_interval, false);
            } else {
                let changed = last_value.borrow().as_deref() != Some(value);
                if changed {
                    label.set_text(value);
                    *last_value.borrow_mut() = Some(value.to_string());
                }
                refresh_backoff
                    .borrow_mut()
                    .note_success(Instant::now(), base_interval, changed);
            }
        });
    }

    fn next_refresh_in(&self, now: Instant) -> Option<Duration> {
        if !self.root.is_visible() {
            return None;
        }
        if self.is_calendar {
            return self
                .calendar_next_due
                .get()
                .map(|due| due.saturating_duration_since(now))
                .or(Some(Duration::ZERO));
        }
        if self.inflight.get() {
            return Some(Duration::from_millis(250));
        }
        self.refresh_backoff
            .borrow()
            .next_due_in(now)
            .or(Some(Duration::ZERO))
    }

    fn refresh_plugin(&self, plugin: &WidgetPluginConfig, base_interval: Duration) {
        self.inflight.set(true);
        let command = plugin.command.clone();
        let timeout = Duration::from_millis(plugin.timeout_ms);
        let output_limits = PluginOutputLimits {
            max_output_bytes: plugin.max_output_bytes,
        };
        let rx = run_command_capture_with_timeout_async(&command, timeout);
        let title_label = self.title_label.clone();
        let body_label = self.body_label.clone();
        let inflight = self.inflight.clone();
        let last_value = self.last_value.clone();
        let refresh_backoff = self.refresh_backoff.clone();
        glib::MainContext::default().spawn_local(async move {
            let output = match rx.recv().await {
                Ok(output) => output,
                Err(_) => {
                    inflight.set(false);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            inflight.set(false);
            let output = match output {
                Ok(output) => output,
                Err(err) => {
                    warn!(command = %command, ?err, "card plugin command failed");
                    apply_cached_value(&body_label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            if !output.status.success() {
                warn!(command = %command, "card plugin command returned non-zero status");
                apply_cached_value(&body_label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
            }

            let parsed = match parse_card_plugin_payload(&output.stdout, output_limits) {
                Ok(parsed) => parsed,
                Err(err) => {
                    warn!(command = %command, %err, "failed to parse card plugin payload");
                    apply_cached_value(&body_label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            if let Some(title) = parsed.title.as_deref() {
                if title_label.text().as_str() != title {
                    title_label.set_text(title);
                }
            }
            let changed = if last_value.borrow().as_deref() != Some(parsed.text.as_str()) {
                body_label.set_text(&parsed.text);
                *last_value.borrow_mut() = Some(parsed.text);
                true
            } else {
                false
            };
            refresh_backoff
                .borrow_mut()
                .note_success(Instant::now(), base_interval, changed);
        });
    }
}
