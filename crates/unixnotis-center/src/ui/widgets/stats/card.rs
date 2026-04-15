//! Stat refresh and label update logic

use std::time::{Duration, Instant};

use gtk::glib;
use gtk::prelude::*;
use tracing::warn;
use unixnotis_core::{PanelDebugLevel, WidgetPluginConfig};

use super::super::plugin::{parse_stat_plugin_payload, PluginOutputLimits};
use super::super::utils::{run_command_capture_async, run_command_capture_with_timeout_async};
use super::apply_cached_value;
use super::StatItem;
use crate::debug;

impl StatItem {
    pub(super) fn refresh(&self, base_interval: Duration, force: bool) {
        if !self.root.is_visible() {
            return;
        }
        let now = Instant::now();
        // Skip refresh when the backoff window has not elapsed
        if !self.refresh_backoff.borrow().should_refresh(now, force) {
            return;
        }
        debug::log(PanelDebugLevel::Verbose, || {
            format!("stat refresh: {}", self.config.label)
        });
        if self.inflight.get() {
            return;
        }
        if let Some(plugin) = self.config.plugin.as_ref() {
            // Plugin source has higher priority than legacy cmd and builtin paths
            self.refresh_plugin(plugin, base_interval);
            return;
        }
        if let Some(builtin) = self.builtin.borrow_mut().take() {
            self.refresh_builtin(builtin, base_interval);
            return;
        }

        let Some(cmd) = self.config.cmd.as_ref() else {
            // Cards with no source fall back to the placeholder instead of spinning forever
            let changed = self.apply_value("n/a");
            self.refresh_backoff
                .borrow_mut()
                .note_success(Instant::now(), base_interval, changed);
            return;
        };
        self.inflight.set(true);
        let cmd = cmd.clone();
        let rx = run_command_capture_async(&cmd);
        let label = self.value_label.clone();
        let inflight = self.inflight.clone();
        let last_value = self.last_value.clone();
        let refresh_backoff = self.refresh_backoff.clone();
        glib::MainContext::default().spawn_local(async move {
            // Receive first so broken worker paths do not leave the card stuck in-flight
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
                    warn!(?cmd, ?err, "stat command failed");
                    apply_cached_value(&label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            if !output.status.success() {
                warn!(?cmd, "stat command failed");
                apply_cached_value(&label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let value = stdout.trim();
            if value.is_empty() {
                // Empty command output keeps the last good value on screen
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
        if !self.root.is_visible() {
            return None;
        }
        if self.inflight.get() {
            // Keep a short retry window while a command is still running
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
        let label = self.value_label.clone();
        let inflight = self.inflight.clone();
        let last_value = self.last_value.clone();
        let refresh_backoff = self.refresh_backoff.clone();
        glib::MainContext::default().spawn_local(async move {
            // Plugin output uses the same cache rules as plain commands
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
                    warn!(command = %command, ?err, "stat plugin command failed");
                    apply_cached_value(&label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            if !output.status.success() {
                warn!(command = %command, "stat plugin command returned non-zero status");
                apply_cached_value(&label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
            }

            let parsed = match parse_stat_plugin_payload(&output.stdout, output_limits) {
                Ok(parsed) => parsed,
                Err(err) => {
                    warn!(command = %command, %err, "failed to parse stat plugin payload");
                    apply_cached_value(&label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            let changed = if last_value.borrow().as_deref() != Some(parsed.text.as_str()) {
                label.set_text(&parsed.text);
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

    pub(super) fn apply_value(&self, value: &str) -> bool {
        if self.last_value.borrow().as_deref() == Some(value) {
            return false;
        }
        // Cache and label are updated together so later fallback reads stay honest
        self.value_label.set_text(value);
        *self.last_value.borrow_mut() = Some(value.to_string());
        true
    }
}
