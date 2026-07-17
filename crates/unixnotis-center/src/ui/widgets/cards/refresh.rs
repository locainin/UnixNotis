//! Command and plugin refresh lifecycle for card widgets

use std::time::{Duration, Instant};

use gtk::glib;
use gtk::prelude::*;
use tracing::warn;
use unixnotis_core::{PanelDebugLevel, WidgetPluginConfig};

use super::common::apply_cached_value;
use super::CardItem;
use crate::diagnostics::panel_debug as debug;
use crate::ui::widgets::plugin::{parse_card_plugin_payload, PluginOutputLimits};
use crate::ui::widgets::utils::{
    run_command_capture_async, run_command_capture_with_timeout_async, INFLIGHT_REFRESH_RECHECK,
};

impl CardItem {
    pub(super) fn refresh(&self, base_interval: Duration, force: bool) {
        // Calendar cards use a date boundary instead of command polling
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
        // Hidden cards do not spend process or plugin resources
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
        // One in-flight request per card prevents slow commands from piling up
        if self.inflight.get() {
            return;
        }
        if let Some(plugin) = self.config.plugin.as_ref() {
            self.refresh_plugin(plugin, base_interval);
            return;
        }
        // Static cards still advance backoff so the scheduler remains calm
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
        // Completion returns to the GLib owner before touching labels
        glib::MainContext::default().spawn_local(async move {
            let output = if let Ok(output) = rx.recv().await {
                output
            } else {
                inflight.set(false);
                refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
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
            // Lossy decoding keeps a malformed helper from breaking the panel loop
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

    pub(super) fn next_refresh_in(&self, now: Instant) -> Option<Duration> {
        // The panel scheduler asks every visible card for its nearest deadline
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
            // Keep the UI scheduler calm while async card commands are already in flight
            return Some(INFLIGHT_REFRESH_RECHECK);
        }
        self.refresh_backoff
            .borrow()
            .next_due_in(now)
            .or(Some(Duration::ZERO))
    }

    fn refresh_plugin(&self, plugin: &WidgetPluginConfig, base_interval: Duration) {
        // Plugin limits come from validated config and are enforced by the parser
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
        // Worker output crosses back to the main context before widget mutation
        glib::MainContext::default().spawn_local(async move {
            let output = if let Ok(output) = rx.recv().await {
                output
            } else {
                inflight.set(false);
                refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
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

            // Parsing validates the versioned payload before either label changes
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
            let changed = if last_value.borrow().as_deref() == Some(parsed.text.as_str()) {
                false
            } else {
                body_label.set_text(&parsed.text);
                *last_value.borrow_mut() = Some(parsed.text);
                true
            };
            refresh_backoff
                .borrow_mut()
                .note_success(Instant::now(), base_interval, changed);
        });
    }
}
