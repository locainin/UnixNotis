//! Slow-job jitter queue

use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tracing::warn;

use super::{dispatch_ready_job, CommandJob, CommandWorker};

// Keep slow-job wait time out of worker threads
const DELAYED_SLOW_CAPACITY: usize = 128;

pub(super) struct DelayedJob {
    // Run after this time
    pub(super) ready_at: Instant,
    // Break ties for the same time
    pub(super) seq: u64,
    pub(super) job: CommandJob,
}

pub(super) struct DelayedState {
    // Jobs still waiting on jitter
    pub(super) pending: Vec<DelayedJob>,
    // Next tie-break value
    pub(super) next_seq: u64,
}

pub(super) struct DelayedSlowQueue {
    pub(super) state: Mutex<DelayedState>,
    pub(super) wake: Condvar,
}

impl DelayedSlowQueue {
    pub(super) fn global() -> &'static Self {
        static QUEUE: OnceLock<DelayedSlowQueue> = OnceLock::new();
        // One delay queue is enough here
        QUEUE.get_or_init(|| Self {
            state: Mutex::new(DelayedState {
                pending: Vec::new(),
                next_seq: 0,
            }),
            wake: Condvar::new(),
        })
    }

    pub(super) fn ensure_drain_thread(&'static self, worker: &'static CommandWorker) {
        static STARTED: OnceLock<()> = OnceLock::new();
        STARTED.get_or_init(|| {
            let queue = self;
            if let Err(err) = std::thread::Builder::new()
                .name("unixnotis-command-delayed".to_string())
                .spawn(move || queue.drain_loop(worker))
            {
                warn!(?err, "failed to spawn delayed command scheduler");
            }
        });
    }

    pub(super) fn submit(&self, job: CommandJob, jitter: Duration) -> Result<(), CommandJob> {
        // Set the wake time before worker dispatch
        let ready_at = Instant::now() + jitter;
        let Ok(mut state) = self.state.lock() else {
            // A poisoned queue cannot safely accept ownership of another job
            return Err(job);
        };
        try_enqueue_delayed_job(&mut state, job, ready_at, DELAYED_SLOW_CAPACITY)?;
        self.wake.notify_one();
        Ok(())
    }

    fn drain_loop(&'static self, worker: &'static CommandWorker) {
        loop {
            let job = {
                let mut state = self.state.lock().expect("delayed slow queue lock poisoned");
                loop {
                    let now = Instant::now();
                    if let Some(index) = next_ready_delayed_job_index(&state.pending, now) {
                        // Remove the due job before dispatch
                        break state.pending.swap_remove(index).job;
                    }

                    let Some(delay) = next_delayed_wake(&state.pending, now) else {
                        // Wait for more delayed work
                        state = self
                            .wake
                            .wait(state)
                            .expect("delayed slow queue wait lock poisoned");
                        continue;
                    };

                    // Wake again when the next job is due
                    let (guard, _) = self
                        .wake
                        .wait_timeout(state, delay)
                        .expect("delayed slow queue timeout wait lock poisoned");
                    state = guard;
                }
            };

            // Jitter is done, so hand it to the workers
            dispatch_ready_job(worker, job);
        }
    }
}

pub(super) fn try_enqueue_delayed_job(
    state: &mut DelayedState,
    job: CommandJob,
    ready_at: Instant,
    capacity: usize,
) -> Result<(), CommandJob> {
    // Keep this queue bounded too
    if state.pending.len() >= capacity {
        return Err(job);
    }
    // Keep same-time jobs stable
    let seq = state.next_seq;
    state.next_seq = state.next_seq.wrapping_add(1);
    state.pending.push(DelayedJob { ready_at, seq, job });
    Ok(())
}

pub(super) fn next_ready_delayed_job_index(pending: &[DelayedJob], now: Instant) -> Option<usize> {
    // Earliest due job wins
    pending
        .iter()
        .enumerate()
        .filter(|(_, job)| job.ready_at <= now)
        .min_by_key(|(_, job)| (job.ready_at, job.seq))
        .map(|(index, _)| index)
}

pub(super) fn next_delayed_wake(pending: &[DelayedJob], now: Instant) -> Option<Duration> {
    // Only wait until the next due job
    pending
        .iter()
        .map(|job| job.ready_at.saturating_duration_since(now))
        .min()
}
