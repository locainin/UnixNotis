//! Arbitrary command refresh handling

use std::time::{Duration, Instant};

use gtk::glib;
use tracing::warn;
use unixnotis_core::CommandSpec;

use super::super::{render::apply_cached_value, StatItem};
use crate::ui::widgets::utils::run_command_capture_async;

impl StatItem {
    pub(super) fn refresh_command(&self, command: &CommandSpec, base_interval: Duration) {
        self.inflight.set(true);
        let command = command.clone();
        let rx = run_command_capture_async(&command);
        let label = self.value_label.clone();
        let inflight = self.inflight.clone();
        let last_value = self.last_value.clone();
        let refresh_backoff = self.refresh_backoff.clone();

        glib::MainContext::default().spawn_local(async move {
            // Receive first so a broken worker cannot leave the card in flight
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
                    warn!(?command, ?error, "stat command failed");
                    apply_cached_value(&label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            if !output.status.success() {
                warn!(?command, "stat command failed");
                apply_cached_value(&label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let value = stdout.trim();
            if value.is_empty() {
                // Empty output preserves the last good value
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
}
