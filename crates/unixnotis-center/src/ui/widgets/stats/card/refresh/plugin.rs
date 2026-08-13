//! Plugin refresh handling

use std::time::{Duration, Instant};

use gtk::glib;
use tracing::warn;
use unixnotis_core::WidgetPluginConfig;

use super::super::{render::apply_cached_value, StatItem};
use crate::ui::widgets::command_runtime::command::run_command_capture_with_timeout_async;
use crate::ui::widgets::plugin::{parse_stat_plugin_payload, PluginOutputLimits};

impl StatItem {
    pub(super) fn refresh_plugin(&self, plugin: &WidgetPluginConfig, base_interval: Duration) {
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
            // Plugins use the same cache and backoff policy as commands
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
                Err(error) => {
                    warn!(command = %command, ?error, "stat plugin command failed");
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
                Err(error) => {
                    warn!(command = %command, %error, "failed to parse stat plugin payload");
                    apply_cached_value(&label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            let changed = if last_value.borrow().as_deref() == Some(parsed.text.as_str()) {
                false
            } else {
                label.set_text(&parsed.text);
                *last_value.borrow_mut() = Some(parsed.text);
                true
            };
            refresh_backoff
                .borrow_mut()
                .note_success(Instant::now(), base_interval, changed);
        });
    }
}
