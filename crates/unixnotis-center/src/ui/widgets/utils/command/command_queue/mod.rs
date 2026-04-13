//! Command queue entry and worker flow

mod coalesced;
mod delayed;
mod metrics;
#[cfg(test)]
mod tests;

use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crossbeam_channel as channel;
use tracing::warn;
use unixnotis_core::{util, PanelDebugLevel};

use crate::debug;

use self::coalesced::CoalescedRefreshQueue;
use self::delayed::DelayedSlowQueue;
use self::metrics::CommandQueueMetrics;
use super::command_exec::{build_command_runtime, run_command_with_timeout};
use super::{CommandKind, CommandPlan};

const COMMAND_WORKERS: usize = 2;
// Keep backlog capped
const COMMAND_QUEUE_CAPACITY: usize = 128;
// Keep action overflow capped too
const COMMAND_FALLBACK_QUEUE_CAPACITY: usize = 32;
// Slow down repeat warning logs
const COMMAND_QUEUE_WARN_INTERVAL_SECS: u64 = 5;

pub(super) struct CommandJob {
    // Command text for this run
    pub(super) cmd: String,
    pub(super) plan: CommandPlan,
    pub(super) respond: Option<async_channel::Sender<Result<std::process::Output, std::io::Error>>>,
    // Used to split wait time from run time
    pub(super) queued_at: Instant,
}

pub(super) struct CommandWorker {
    // Main queue for worker threads
    pub(super) tx: channel::Sender<CommandJob>,
    // True when worker threads could not start
    inline_fallback: bool,
}

impl CommandWorker {
    fn global() -> &'static CommandWorker {
        static WORKER: OnceLock<CommandWorker> = OnceLock::new();
        // One shared queue keeps one path for dispatch
        WORKER.get_or_init(|| CommandWorker::new(COMMAND_WORKERS))
    }

    fn new(worker_count: usize) -> Self {
        // Cap queue growth during bursts
        let (tx, rx) = channel::bounded(COMMAND_QUEUE_CAPACITY);
        let mut spawned = 0usize;
        for idx in 0..worker_count.max(1) {
            let rx = rx.clone();
            match std::thread::Builder::new()
                .name(format!("unixnotis-command-worker-{idx}"))
                .spawn(move || run_worker(rx))
            {
                Ok(_) => spawned += 1,
                Err(err) => {
                    warn!(?err, "failed to spawn command worker thread");
                }
            }
        }
        if spawned == 0 {
            // Last resort when no worker could start
            warn!("no command worker threads available; falling back to inline execution");
        }
        Self {
            tx,
            inline_fallback: spawned == 0,
        }
    }
}

pub(super) fn enqueue_command(
    cmd: String,
    plan: CommandPlan,
    respond: Option<async_channel::Sender<Result<std::process::Output, std::io::Error>>>,
) {
    let worker = CommandWorker::global();
    let metrics = CommandQueueMetrics::global();
    metrics.record_enqueued();
    if worker.inline_fallback {
        // Try one extra thread before using inline work
        if std::thread::Builder::new()
            .name("unixnotis-command-fallback".to_string())
            .spawn({
                let job_for_thread = CommandJob {
                    cmd: cmd.clone(),
                    plan,
                    respond: respond.clone(),
                    queued_at: Instant::now(),
                };
                move || handle_job(job_for_thread, None)
            })
            .is_err()
        {
            warn!("failed to spawn fallback command worker; running inline");
            handle_job(
                CommandJob {
                    cmd,
                    plan,
                    respond,
                    queued_at: Instant::now(),
                },
                None,
            );
        }
        return;
    }

    let job = CommandJob {
        cmd,
        plan,
        respond,
        queued_at: Instant::now(),
    };

    // Only slow jobs use jitter
    // Keep that wait out of the workers
    let jitter = job.plan.jitter();
    if !jitter.is_zero() {
        // Slow jobs wait here before worker dispatch
        let delayed = DelayedSlowQueue::global();
        delayed.ensure_drain_thread(worker);
        match delayed.submit(job, jitter) {
            Ok(()) => {
                metrics.record_delayed();
                return;
            }
            Err(job) => {
                // If full, skip jitter and run now
                metrics.record_delayed_bypassed();
                if should_warn_queue_full() {
                    warn!(
                        snapshot = %metrics.snapshot().summary(),
                        "delayed slow queue full; dispatching command without jitter"
                    );
                }
                dispatch_ready_job(worker, job);
                return;
            }
        }
    }

    dispatch_ready_job(worker, job);
}

pub(super) fn dispatch_ready_job(worker: &CommandWorker, job: CommandJob) {
    let metrics = CommandQueueMetrics::global();
    // Normal worker entry point
    match worker.tx.try_send(job) {
        Ok(()) => {}
        Err(channel::TrySendError::Full(job)) => {
            // Keep memory bounded and actions responsive
            if job.plan.kind == CommandKind::Action {
                metrics.record_action_overflow();
                if should_warn_queue_full() {
                    warn!(
                        snapshot = %metrics.snapshot().summary(),
                        "command queue full; routing action to fallback worker"
                    );
                }
                if FallbackWorker::global().submit(job).is_err() {
                    metrics.record_action_dropped();
                    if should_warn_queue_full() {
                        warn!(
                            snapshot = %metrics.snapshot().summary(),
                            "fallback command queue full; dropping action"
                        );
                    }
                }
            } else {
                metrics.record_refresh_overflow();
                if should_warn_queue_full() {
                    warn!(
                        snapshot = %metrics.snapshot().summary(),
                        "command queue full; coalescing refresh command"
                    );
                }
                let coalescer = CoalescedRefreshQueue::global();
                coalescer.ensure_drain_thread(worker.tx.clone());
                let outcome = coalescer.enqueue(job);
                if outcome.replaced_existing {
                    metrics.record_refresh_replaced();
                }
                if outcome.evicted_oldest {
                    metrics.record_refresh_evicted();
                }
            }
        }
        Err(channel::TrySendError::Disconnected(_job)) => {
            warn!("command worker channel closed");
        }
    }
}

struct FallbackWorker {
    tx: channel::Sender<CommandJob>,
}

impl FallbackWorker {
    fn global() -> &'static FallbackWorker {
        static WORKER: OnceLock<FallbackWorker> = OnceLock::new();
        // One extra queue for action overflow
        WORKER.get_or_init(|| FallbackWorker::new(COMMAND_FALLBACK_QUEUE_CAPACITY))
    }

    fn new(capacity: usize) -> Self {
        // Keep overflow work off the GTK thread
        let (tx, rx) = channel::bounded(capacity);
        if std::thread::Builder::new()
            .name("unixnotis-command-fallback".to_string())
            .spawn(move || run_worker(rx))
            .is_err()
        {
            warn!("failed to spawn fallback command worker");
        }
        Self { tx }
    }

    fn submit(&self, job: CommandJob) -> Result<(), channel::TrySendError<CommandJob>> {
        self.tx.try_send(job)
    }
}

fn should_warn_queue_full() -> bool {
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

    static LAST_WARN: AtomicU64 = AtomicU64::new(0);
    // Best-effort warning throttle
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last = LAST_WARN.load(AtomicOrdering::Relaxed);
    if now.saturating_sub(last) >= COMMAND_QUEUE_WARN_INTERVAL_SECS {
        // Ordering does not matter here
        LAST_WARN.store(now, AtomicOrdering::Relaxed);
        return true;
    }
    false
}

fn run_worker(rx: channel::Receiver<CommandJob>) {
    // Reuse one runtime per worker
    let runtime = build_command_runtime();
    for job in rx.iter() {
        // Order only holds inside one worker
        handle_job(job, runtime.as_ref());
    }
}

fn handle_job(job: CommandJob, runtime: Option<&tokio::runtime::Runtime>) {
    let cmd_snip = util::log_snippet(&job.cmd);
    // Wait time includes queue time and slow-job jitter
    let queue_wait_ms = job.queued_at.elapsed().as_millis();
    debug::log(PanelDebugLevel::Verbose, || {
        format!(
            "command start kind={:?} queue_wait_ms={} cmd={}",
            job.plan.kind, queue_wait_ms, cmd_snip
        )
    });
    // Execution time starts once the worker begins the command
    let exec_started = Instant::now();
    let result = run_command_with_timeout(&job.cmd, job.plan.timeout(), runtime);
    let exec_ms = exec_started.elapsed().as_millis();
    // Total time keeps the full wait easy to compare in logs
    let total_ms = job.queued_at.elapsed().as_millis();
    if let Some(tx) = job.respond {
        // Direct responses skip warning paths because the caller owns error handling
        let _ = tx.send_blocking(result);
        return;
    }
    match result {
        Ok(output) => {
            if !output.status.success() {
                warn!(command = %cmd_snip, "command returned non-zero status");
                debug::log(PanelDebugLevel::Warn, || {
                    format!(
                        "command failed kind={:?} status={:?} queue_wait_ms={} exec_ms={} total_ms={}",
                        job.plan.kind,
                        output.status.code(),
                        queue_wait_ms,
                        exec_ms,
                        total_ms
                    )
                });
            } else {
                debug::log(PanelDebugLevel::Verbose, || {
                    format!(
                        "command ok kind={:?} status={:?} queue_wait_ms={} exec_ms={} total_ms={}",
                        job.plan.kind,
                        output.status.code(),
                        queue_wait_ms,
                        exec_ms,
                        total_ms
                    )
                });
            }
        }
        Err(err) => {
            warn!(command = %cmd_snip, ?err, "command failed");
            debug::log(PanelDebugLevel::Warn, || {
                format!(
                    "command error kind={:?} queue_wait_ms={} exec_ms={} total_ms={} err={err}",
                    job.plan.kind, queue_wait_ms, exec_ms, total_ms
                )
            });
        }
    }
}
