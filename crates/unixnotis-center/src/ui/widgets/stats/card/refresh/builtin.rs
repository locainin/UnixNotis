//! Individual built-in refresh handling

use std::time::{Duration, Instant};

use gtk::glib;

use super::super::{render::apply_cached_value, StatItem};
use crate::ui::widgets::stats::builtin::worker::{
    BuiltinJob, BuiltinSample, BuiltinWorker, SubmitOutcome,
};
use crate::ui::widgets::stats::builtin::BuiltinStat;

impl StatItem {
    pub(super) fn refresh_builtin(&self, builtin: BuiltinStat, base_interval: Duration) {
        self.inflight.set(true);
        let (tx, rx) = async_channel::bounded(1);
        let fallback = builtin.clone();
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
                self.restore_builtin_sample(BuiltinSample::read(fallback), base_interval);
                return;
            }
        }

        let item = self.clone();
        glib::MainContext::default().spawn_local(async move {
            // Restore reader state on every exit path
            let result = rx.recv().await;
            let Ok(sample) = result else {
                item.restore_builtin_error(fallback, base_interval);
                return;
            };
            item.restore_builtin_sample(sample, base_interval);
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

    pub(in crate::ui::widgets::stats) fn restore_builtin_sample(
        &self,
        sample: BuiltinSample,
        base_interval: Duration,
    ) {
        let BuiltinSample { stat, value } = sample;
        let Some(value) = value else {
            // Reader failure preserves the last good value and uses error backoff
            apply_cached_value(&self.value_label, &self.last_value);
            self.restore_builtin_error(stat, base_interval);
            return;
        };

        self.inflight.set(false);
        *self.builtin.borrow_mut() = Some(stat);
        if value.is_empty() {
            apply_cached_value(&self.value_label, &self.last_value);
            self.refresh_backoff
                .borrow_mut()
                .note_success(Instant::now(), base_interval, false);
            return;
        }

        let changed = self.last_value.borrow().as_deref() != Some(value.as_str());
        if changed {
            self.value_label.set_text(&value);
            *self.last_value.borrow_mut() = Some(value);
        }
        self.refresh_backoff
            .borrow_mut()
            .note_success(Instant::now(), base_interval, changed);
    }
}
