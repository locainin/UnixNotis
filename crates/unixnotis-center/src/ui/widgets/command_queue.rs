//! Command worker queues and scheduling logic.
//!
//! Owns thread spawning, backpressure, and dispatch so command execution
//! stays responsive under load without unbounded memory growth.

use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crossbeam_channel as channel;
use tracing::warn;
use unixnotis_core::{util, PanelDebugLevel};

use crate::debug;

use super::command_exec::{build_command_runtime, run_command_with_timeout};
use super::{CommandKind, CommandPlan};

const COMMAND_WORKERS: usize = 2;
// Bound the command queue to avoid unbounded memory growth during stalls.
const COMMAND_QUEUE_CAPACITY: usize = 128;
// Fallback queue keeps user actions flowing without spawning unbounded threads.
const COMMAND_FALLBACK_QUEUE_CAPACITY: usize = 32;
// Rate-limit queue saturation warnings to avoid log spam under misconfiguration.
const COMMAND_QUEUE_WARN_INTERVAL_SECS: u64 = 5;

struct CommandJob {
    cmd: String,
    plan: CommandPlan,
    respond: Option<async_channel::Sender<Result<std::process::Output, std::io::Error>>>,
}

struct CommandWorker {
    tx: channel::Sender<CommandJob>,
    inline_fallback: bool,
}

impl CommandWorker {
    fn global() -> &'static CommandWorker {
        static WORKER: OnceLock<CommandWorker> = OnceLock::new();
        WORKER.get_or_init(|| CommandWorker::new(COMMAND_WORKERS))
    }

    fn new(worker_count: usize) -> Self {
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
    if worker.inline_fallback {
        // Fall back to a dedicated worker thread to avoid blocking the GTK loop.
        if std::thread::Builder::new()
            .name("unixnotis-command-fallback".to_string())
            .spawn({
                let job_for_thread = CommandJob {
                    cmd: cmd.clone(),
                    plan,
                    respond: respond.clone(),
                };
                move || handle_job(job_for_thread, None)
            })
            .is_err()
        {
            warn!("failed to spawn fallback command worker; running inline");
            handle_job(CommandJob { cmd, plan, respond }, None);
        }
        return;
    }
    let job = CommandJob { cmd, plan, respond };
    match worker.tx.try_send(job) {
        Ok(()) => {}
        Err(channel::TrySendError::Full(job)) => {
            // Avoid unbounded memory growth; prefer executing user actions immediately.
            if job.plan.kind == CommandKind::Action {
                if should_warn_queue_full() {
                    warn!("command queue full; routing action to fallback worker");
                }
                if FallbackWorker::global().submit(job).is_err() && should_warn_queue_full() {
                    warn!("fallback command queue full; dropping action");
                }
            } else if should_warn_queue_full() {
                warn!("command queue full; dropping refresh command");
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
        WORKER.get_or_init(|| FallbackWorker::new(COMMAND_FALLBACK_QUEUE_CAPACITY))
    }

    fn new(capacity: usize) -> Self {
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last = LAST_WARN.load(AtomicOrdering::Relaxed);
    if now.saturating_sub(last) >= COMMAND_QUEUE_WARN_INTERVAL_SECS {
        LAST_WARN.store(now, AtomicOrdering::Relaxed);
        return true;
    }
    false
}

fn run_worker(rx: channel::Receiver<CommandJob>) {
    // Each worker owns a small current-thread runtime to avoid per-command OS threads.
    let runtime = build_command_runtime();
    for job in rx.iter() {
        handle_job(job, runtime.as_ref());
    }
}

fn handle_job(job: CommandJob, runtime: Option<&tokio::runtime::Runtime>) {
    let cmd_snip = util::log_snippet(&job.cmd);
    debug::log(PanelDebugLevel::Verbose, || {
        format!("command start kind={:?} cmd={}", job.plan.kind, cmd_snip)
    });
    let started = Instant::now();
    let jitter = job.plan.jitter();
    if !jitter.is_zero() {
        std::thread::sleep(jitter);
    }
    let result = run_command_with_timeout(&job.cmd, job.plan.timeout(), runtime);
    let elapsed_ms = started.elapsed().as_millis();
    if let Some(tx) = job.respond {
        let _ = tx.send_blocking(result);
        return;
    }
    match result {
        Ok(output) => {
            if !output.status.success() {
                warn!(command = %cmd_snip, "command returned non-zero status");
                debug::log(PanelDebugLevel::Warn, || {
                    format!(
                        "command failed kind={:?} status={:?} elapsed_ms={elapsed_ms}",
                        job.plan.kind,
                        output.status.code()
                    )
                });
            } else {
                debug::log(PanelDebugLevel::Verbose, || {
                    format!(
                        "command ok kind={:?} status={:?} elapsed_ms={elapsed_ms}",
                        job.plan.kind,
                        output.status.code()
                    )
                });
            }
        }
        Err(err) => {
            warn!(command = %cmd_snip, ?err, "command failed");
            debug::log(PanelDebugLevel::Warn, || {
                format!(
                    "command error kind={:?} elapsed_ms={elapsed_ms} err={err}",
                    job.plan.kind
                )
            });
        }
    }
}
