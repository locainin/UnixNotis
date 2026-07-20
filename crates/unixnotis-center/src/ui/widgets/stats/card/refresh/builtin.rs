//! Individual built-in refresh handling

use std::time::{Duration, Instant};

use gtk::glib;

use super::super::{render::apply_cached_value, StatItem};
use crate::ui::widgets::stats::builtin::worker::{BuiltinJob, BuiltinWorker, SubmitOutcome};
use crate::ui::widgets::stats::builtin::BuiltinStat;

impl StatItem {
    pub(super) fn refresh_builtin(&self, builtin: BuiltinStat, base_interval: Duration) {
        self.inflight.set(true);
        let (tx, rx) = async_channel::bounded(1);
        let mut fallback = builtin.clone();
        let worker = BuiltinWorker::global();

        match worker.submit(BuiltinJob {
            stat: builtin,
            respond: tx,
        }) {
            SubmitOutcome::Submitted => {}
            SubmitOutcome::QueueFull => {
                // Queue pressure remains non-blocking on the GTK thread
                self.restore_builtin_error(fallback, base_interval);
                return;
            }
            SubmitOutcome::WorkerUnavailable => {
                // Inline fallback keeps built-in cards available after startup failure
                let value = fallback.read().unwrap_or_else(|| "n/a".to_string());
                self.restore_builtin_value(fallback, &value, base_interval);
                return;
            }
        }

        let item = self.clone();
        glib::MainContext::default().spawn_local(async move {
            // Restore reader state on every exit path
            let result = rx.recv().await;
            let Ok((builtin, value)) = result else {
                item.restore_builtin_error(fallback, base_interval);
                return;
            };
            item.restore_builtin_value(builtin, &value, base_interval);
        });
    }

    pub(in crate::ui::widgets::stats) fn restore_builtin_error(
        &self,
        builtin: BuiltinStat,
        base_interval: Duration,
    ) {
        self.inflight.set(false);
        *self.builtin.borrow_mut() = Some(builtin);
        self.refresh_backoff
            .borrow_mut()
            .note_error(Instant::now(), base_interval);
    }

    pub(in crate::ui::widgets::stats) fn restore_builtin_value(
        &self,
        builtin: BuiltinStat,
        value: &str,
        base_interval: Duration,
    ) {
        self.inflight.set(false);
        *self.builtin.borrow_mut() = Some(builtin);
        if value.is_empty() {
            apply_cached_value(&self.value_label, &self.last_value);
            self.refresh_backoff
                .borrow_mut()
                .note_success(Instant::now(), base_interval, false);
            return;
        }

        let changed = self.last_value.borrow().as_deref() != Some(value);
        if changed {
            self.value_label.set_text(value);
            *self.last_value.borrow_mut() = Some(value.to_string());
        }
        self.refresh_backoff
            .borrow_mut()
            .note_success(Instant::now(), base_interval, changed);
    }
}
