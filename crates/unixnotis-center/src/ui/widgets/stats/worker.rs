//! Builtin stat worker and builtin refresh helpers

use std::thread;
use std::time::Instant;

use crossbeam_channel::TrySendError;
use gtk::glib;
use tracing::warn;

use super::{
    apply_cached_value, BuiltinStat, BuiltinStatJob, BuiltinStatWorker, BuiltinSubmitOutcome,
    StatItem,
};

impl BuiltinStatWorker {
    // Limit queued jobs to avoid unbounded growth if refresh is faster than the worker
    const QUEUE_CAPACITY: usize = 32;

    // Single worker avoids per-refresh thread churn while keeping UI updates async
    pub(super) fn global() -> &'static Self {
        static WORKER: std::sync::OnceLock<BuiltinStatWorker> = std::sync::OnceLock::new();
        WORKER.get_or_init(Self::new)
    }

    fn new() -> Self {
        Self::new_with_capacity(Self::QUEUE_CAPACITY, true)
    }

    fn new_with_capacity(capacity: usize, spawn_workers: bool) -> Self {
        let (tx, rx) = crossbeam_channel::bounded::<BuiltinStatJob>(capacity);
        #[cfg(test)]
        let receiver_guard = rx.clone();
        let inline_fallback = if spawn_workers {
            // One worker thread is enough because builtin reads are short and serialized
            let spawn = thread::Builder::new()
                .name("unixnotis-builtin-stats".to_string())
                .spawn(move || {
                    for mut job in rx.iter() {
                        let value = job.stat.read().unwrap_or_else(|| "n/a".to_string());
                        let _ = job.respond.send_blocking((job.stat, value));
                    }
                });
            spawn.is_err()
        } else {
            true
        };
        if inline_fallback {
            warn!("builtin stats worker unavailable; using inline reads");
        }

        Self {
            tx,
            inline_fallback,
            #[cfg(test)]
            receiver_guard,
        }
    }

    pub(super) fn submit(&self, job: BuiltinStatJob) -> BuiltinSubmitOutcome {
        if self.inline_fallback {
            return BuiltinSubmitOutcome::WorkerUnavailable;
        }
        // Avoid blocking the UI thread when the worker queue is saturated
        match self.tx.try_send(job) {
            Ok(()) => BuiltinSubmitOutcome::Submitted,
            Err(TrySendError::Full(_job)) => BuiltinSubmitOutcome::QueueFull,
            // Disconnected queue means the worker path is no longer usable
            Err(TrySendError::Disconnected(_job)) => BuiltinSubmitOutcome::WorkerUnavailable,
        }
    }
}

#[cfg(test)]
impl BuiltinStatWorker {
    pub(super) fn new_for_tests(capacity: usize) -> Self {
        let (tx, rx) = crossbeam_channel::bounded::<BuiltinStatJob>(capacity);
        Self {
            tx,
            inline_fallback: false,
            receiver_guard: rx,
        }
    }
}

impl StatItem {
    pub(super) fn refresh_builtin(&self, builtin: BuiltinStat, base_interval: std::time::Duration) {
        // Temporarily take builtin state to prevent overlapping reads
        self.inflight.set(true);
        let (tx, rx) = async_channel::bounded(1);
        let mut fallback = builtin.clone();
        let worker = BuiltinStatWorker::global();
        match worker.submit(BuiltinStatJob {
            stat: builtin,
            respond: tx,
        }) {
            BuiltinSubmitOutcome::Submitted => {}
            BuiltinSubmitOutcome::QueueFull => {
                // Queue saturation should stay non-blocking on the GTK thread
                self.inflight.set(false);
                *self.builtin.borrow_mut() = Some(fallback);
                self.refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
            }
            BuiltinSubmitOutcome::WorkerUnavailable => {
                self.inflight.set(false);
                // Inline fallback keeps builtin stats readable when the worker is missing
                let value = fallback.read().unwrap_or_else(|| "n/a".to_string());
                *self.builtin.borrow_mut() = Some(fallback);
                let changed = self.apply_value(&value);
                self.refresh_backoff.borrow_mut().note_success(
                    Instant::now(),
                    base_interval,
                    changed,
                );
                return;
            }
        }

        let label = self.value_label.clone();
        let inflight = self.inflight.clone();
        let builtin_cell = self.builtin.clone();
        let last_value = self.last_value.clone();
        let refresh_backoff = self.refresh_backoff.clone();
        glib::MainContext::default().spawn_local(async move {
            // Restore builtin state on every exit path so later refreshes can keep working
            let result = rx.recv().await;
            inflight.set(false);
            let Ok((builtin, value)) = result else {
                *builtin_cell.borrow_mut() = Some(fallback);
                refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
            };
            *builtin_cell.borrow_mut() = Some(builtin);
            if value.is_empty() {
                apply_cached_value(&label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_success(Instant::now(), base_interval, false);
            } else if last_value.borrow().as_deref() != Some(&value) {
                label.set_text(&value);
                *last_value.borrow_mut() = Some(value);
                refresh_backoff
                    .borrow_mut()
                    .note_success(Instant::now(), base_interval, true);
            } else {
                refresh_backoff
                    .borrow_mut()
                    .note_success(Instant::now(), base_interval, false);
            }
        });
    }
}
