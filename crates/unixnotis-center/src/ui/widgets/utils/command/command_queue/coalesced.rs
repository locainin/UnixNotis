//! Refresh overflow queue

use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use crossbeam_channel as channel;
use tracing::warn;

use super::CommandJob;
use crate::ui::widgets::utils::command::CommandKind;

// Keep refresh overflow bounded
const COALESCED_REFRESH_CAPACITY: usize = 256;
const COALESCED_RETRY_DELAY_MS: u64 = 25;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(super) struct RefreshCommandKey {
    cmd: String,
    kind: CommandKind,
    timeout_ms: Option<u64>,
}

impl RefreshCommandKey {
    fn from_job(job: &CommandJob) -> Self {
        // Timeout changes the job shape too
        Self {
            cmd: job.cmd.clone(),
            kind: job.plan.kind,
            timeout_ms: job
                .plan
                .timeout_override
                .map(|timeout| timeout.as_millis().min(u64::MAX as u128) as u64),
        }
    }
}

pub(super) struct CoalescedRefreshState {
    // Newest job for each key
    pub(super) pending: HashMap<RefreshCommandKey, CommandJob>,
    // Drain order for keys
    pub(super) order: VecDeque<RefreshCommandKey>,
}

pub(super) struct CoalescedRefreshQueue {
    state: Mutex<CoalescedRefreshState>,
    wake: Condvar,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CoalescedInsertOutcome {
    pub(super) replaced_existing: bool,
    pub(super) evicted_oldest: bool,
}

impl CoalescedRefreshQueue {
    pub(super) fn global() -> &'static Self {
        static QUEUE: OnceLock<CoalescedRefreshQueue> = OnceLock::new();
        // One shared overflow queue
        QUEUE.get_or_init(|| CoalescedRefreshQueue {
            state: Mutex::new(CoalescedRefreshState {
                pending: HashMap::new(),
                order: VecDeque::new(),
            }),
            wake: Condvar::new(),
        })
    }

    pub(super) fn ensure_drain_thread(&'static self, worker_tx: channel::Sender<CommandJob>) {
        static STARTED: OnceLock<()> = OnceLock::new();
        STARTED.get_or_init(|| {
            let queue = self;
            if let Err(err) = std::thread::Builder::new()
                .name("unixnotis-command-coalescer".to_string())
                .spawn(move || queue.drain_loop(worker_tx))
            {
                warn!(?err, "failed to spawn refresh coalescer thread");
            }
        });
    }

    pub(super) fn enqueue(&self, job: CommandJob) -> CoalescedInsertOutcome {
        let mut state = self.state.lock().expect("coalesced refresh lock poisoned");
        // Keep only the newest job for a key
        let outcome = insert_coalesced_job(&mut state, job);
        self.wake.notify_one();
        outcome
    }

    fn drain_loop(&'static self, worker_tx: channel::Sender<CommandJob>) {
        loop {
            let (key, job) = {
                let mut state = self.state.lock().expect("coalesced refresh lock poisoned");
                while state.order.is_empty() {
                    // Wait for more overflow work
                    state = self
                        .wake
                        .wait(state)
                        .expect("coalesced refresh wait lock poisoned");
                }
                let Some(key) = state.order.pop_front() else {
                    continue;
                };
                let Some(job) = state.pending.remove(&key) else {
                    continue;
                };
                (key, job)
            };

            match worker_tx.try_send(job) {
                Ok(()) => {}
                Err(channel::TrySendError::Full(job)) => {
                    // Worker queue is still full, so put it back
                    let mut state = self.state.lock().expect("coalesced refresh lock poisoned");
                    if !state.pending.contains_key(&key)
                        && state.pending.len() >= COALESCED_REFRESH_CAPACITY
                    {
                        if let Some(oldest) = state.order.pop_front() {
                            state.pending.remove(&oldest);
                        }
                    }
                    if !state.pending.contains_key(&key) {
                        state.order.push_front(key.clone());
                    }
                    state.pending.insert(key, job);
                    drop(state);
                    // Small delay keeps this from spinning
                    std::thread::sleep(Duration::from_millis(COALESCED_RETRY_DELAY_MS));
                }
                Err(channel::TrySendError::Disconnected(_job)) => return,
            }
        }
    }
}

pub(super) fn insert_coalesced_job(
    state: &mut CoalescedRefreshState,
    job: CommandJob,
) -> CoalescedInsertOutcome {
    let key = RefreshCommandKey::from_job(&job);
    let replaced_existing = state.pending.contains_key(&key);
    let mut evicted_oldest = false;
    if !replaced_existing {
        if state.pending.len() >= COALESCED_REFRESH_CAPACITY {
            if let Some(oldest) = state.order.pop_front() {
                // Drop the oldest job when full
                state.pending.remove(&oldest);
                evicted_oldest = true;
            }
        }
        // First seen key goes to the back
        state.order.push_back(key.clone());
    }
    // Replacing the old job drops stale refresh work
    state.pending.insert(key, job);
    CoalescedInsertOutcome {
        replaced_existing,
        evicted_oldest,
    }
}
