//! Builtin stat worker and builtin refresh helpers

use std::thread;

use crossbeam_channel::TrySendError;
use gtk::glib;
use tracing::warn;

use super::{
    BuiltinRefreshGroup, BuiltinStat, BuiltinStatJob, BuiltinStatWorker, BuiltinSubmitOutcome,
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
                self.restore_builtin_error(fallback, base_interval);
                return;
            }
            BuiltinSubmitOutcome::WorkerUnavailable => {
                // Inline fallback keeps builtin stats readable when the worker is missing
                let value = fallback.read().unwrap_or_else(|| "n/a".to_string());
                self.restore_builtin_value(fallback, &value, base_interval);
                return;
            }
        }

        let item = self.clone();
        glib::MainContext::default().spawn_local(async move {
            // Restore builtin state on every exit path so later refreshes can keep working
            let result = rx.recv().await;
            let Ok((builtin, value)) = result else {
                item.restore_builtin_error(fallback, base_interval);
                return;
            };
            item.restore_builtin_value(builtin, &value, base_interval);
        });
    }
}

impl BuiltinRefreshGroup {
    pub(super) fn refresh(self, base_interval: std::time::Duration) {
        let (tx, rx) = async_channel::bounded(1);
        let mut fallback = self.stat.clone();
        let worker = BuiltinStatWorker::global();

        match worker.submit(BuiltinStatJob {
            stat: self.stat,
            respond: tx,
        }) {
            BuiltinSubmitOutcome::Submitted => {}
            BuiltinSubmitOutcome::QueueFull => {
                // Restore every grouped item so the next refresh wave can retry cleanly
                for item in self.items {
                    item.restore_builtin_error(fallback.clone(), base_interval);
                }
                return;
            }
            BuiltinSubmitOutcome::WorkerUnavailable => {
                // Inline fallback still samples the source once, then fans the value out to every card
                let value = fallback.read().unwrap_or_else(|| "n/a".to_string());
                for item in self.items {
                    item.restore_builtin_value(fallback.clone(), &value, base_interval);
                }
                return;
            }
        }

        glib::MainContext::default().spawn_local(async move {
            let result = rx.recv().await;
            let Ok((builtin, value)) = result else {
                for item in self.items {
                    item.restore_builtin_error(fallback.clone(), base_interval);
                }
                return;
            };

            // Every grouped card receives the same value and updated reader state clone
            for item in self.items {
                item.restore_builtin_value(builtin.clone(), &value, base_interval);
            }
        });
    }
}
