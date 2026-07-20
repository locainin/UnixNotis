//! Bounded worker for built-in statistic samples

use std::thread;

use crossbeam_channel::TrySendError;
use tracing::warn;

use super::BuiltinStat;

pub(in crate::ui::widgets::stats) struct BuiltinJob {
    // Reader state moves to the worker for one sample
    pub(in crate::ui::widgets::stats) stat: BuiltinStat,
    // One-shot response returns the updated reader and display value
    pub(in crate::ui::widgets::stats) respond: async_channel::Sender<(BuiltinStat, String)>,
}

pub(in crate::ui::widgets::stats) struct BuiltinWorker {
    // Bounded transport prevents refresh waves from growing memory without limit
    pub(in crate::ui::widgets::stats) tx: crossbeam_channel::Sender<BuiltinJob>,
    // Failed startup selects the inline fallback path
    pub(in crate::ui::widgets::stats) inline_fallback: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::ui::widgets::stats) enum SubmitOutcome {
    Submitted,
    QueueFull,
    WorkerUnavailable,
}

impl BuiltinWorker {
    const QUEUE_CAPACITY: usize = 32;

    pub(in crate::ui::widgets::stats) fn global() -> &'static Self {
        static WORKER: std::sync::OnceLock<BuiltinWorker> = std::sync::OnceLock::new();
        WORKER.get_or_init(Self::new)
    }

    fn new() -> Self {
        let (tx, rx) = crossbeam_channel::bounded::<BuiltinJob>(Self::QUEUE_CAPACITY);
        // One thread is enough because built-in reads are short and serialized
        let spawn = thread::Builder::new()
            .name("unixnotis-builtin-stats".to_string())
            .spawn(move || {
                for mut job in &rx {
                    let value = job.stat.read().unwrap_or_else(|| "n/a".to_string());
                    let _ = job.respond.send_blocking((job.stat, value));
                }
            });
        let inline_fallback = spawn.is_err();
        if inline_fallback {
            warn!("builtin stats worker unavailable; using inline reads");
        }

        Self {
            tx,
            inline_fallback,
        }
    }

    pub(in crate::ui::widgets::stats) fn submit(&self, job: BuiltinJob) -> SubmitOutcome {
        if self.inline_fallback {
            return SubmitOutcome::WorkerUnavailable;
        }
        // The GTK thread never waits for queue capacity
        match self.tx.try_send(job) {
            Ok(()) => SubmitOutcome::Submitted,
            Err(TrySendError::Full(_job)) => SubmitOutcome::QueueFull,
            Err(TrySendError::Disconnected(_job)) => SubmitOutcome::WorkerUnavailable,
        }
    }
}
