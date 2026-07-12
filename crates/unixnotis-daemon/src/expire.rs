//! Notification expiration scheduling and timeouts

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tracing::warn;

use crate::daemon::DaemonState;
use unixnotis_core::CloseReason;

/// Commands sent to the expiration scheduler
pub enum ExpirationCommand {
    Schedule { id: u32, deadline: Instant },
    Cancel { id: u32 },
}

/// Asynchronous expiration manager backed by a priority queue
#[derive(Clone)]
pub struct ExpirationScheduler {
    sender: mpsc::UnboundedSender<ExpirationCommand>,
}

impl ExpirationScheduler {
    pub fn start(state: Arc<DaemonState>) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut heap: BinaryHeap<ExpirationItem> = BinaryHeap::new();
            // Tracks the latest deadline per notification to discard stale heap entries
            let mut scheduled: HashMap<u32, Instant> = HashMap::new();
            loop {
                let next_deadline = heap.peek().map(|item| item.deadline);
                if next_deadline.is_none() {
                    let Some(cmd) = receiver.recv().await else {
                        break;
                    };
                    apply_command(cmd, &mut heap, &mut scheduled);
                    continue;
                }
                let Some(deadline) = next_deadline else {
                    continue;
                };

                tokio::select! {
                    Some(cmd) = receiver.recv() => {
                        apply_command(cmd, &mut heap, &mut scheduled);
                        maybe_compact(&mut heap, &scheduled);
                    }
                    _ = tokio::time::sleep_until(deadline.into()) => {
                        let now = Instant::now();
                        while let Some(item) = heap.peek() {
                            if item.deadline > now {
                                break;
                            }
                            let Some(item) = heap.pop() else {
                                break;
                            };
                            let is_current = scheduled
                                .get(&item.id)
                                .map(|deadline| *deadline == item.deadline)
                                .unwrap_or(false);
                            if !is_current {
                                continue;
                            }
                            // Verify the deadline is still current before closing the notification
                            let expiration = {
                                let store = state.store.lock().await;
                                store.expiration_for(item.id)
                            };
                            let is_still_current = expiration
                                .map(|deadline| deadline == item.deadline)
                                .unwrap_or(false);
                            if is_still_current {
                                // Remove the scheduled entry only once the deadline is confirmed
                                // to still be active. This avoids dropping new schedules created
                                // while the expiration task was waiting on the store lock
                                if scheduled.get(&item.id) == Some(&item.deadline) {
                                    scheduled.remove(&item.id);
                                }
                                // Expiration closes must be observable so signal/state failures
                                // are visible in logs instead of being silently ignored
                                if let Err(err) =
                                    state.close_notification(item.id, CloseReason::Expired).await
                                {
                                    warn!(
                                        ?err,
                                        id = item.id,
                                        "failed to close expired notification"
                                    );
                                }
                            } else if scheduled.get(&item.id) == Some(&item.deadline) {
                                // The store no longer expects this deadline (dismissed or updated),
                                // so drop the stale schedule to avoid repeated checks
                                scheduled.remove(&item.id);
                            }
                        }
                        maybe_compact(&mut heap, &scheduled);
                    }
                    else => break,
                };
            }
        });

        Self { sender }
    }

    pub fn schedule(&self, id: u32, deadline: Option<Instant>) {
        let command = match deadline {
            Some(deadline) => ExpirationCommand::Schedule { id, deadline },
            None => ExpirationCommand::Cancel { id },
        };
        if let Err(err) = self.sender.send(command) {
            warn!(?err, "expiration schedule request dropped");
        }
    }

    #[cfg(test)]
    pub(crate) fn channel_for_test() -> (Self, mpsc::UnboundedReceiver<ExpirationCommand>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }
}

#[derive(Debug, Copy, Clone)]
struct ExpirationItem {
    id: u32,
    deadline: Instant,
}

impl PartialEq for ExpirationItem {
    fn eq(&self, other: &Self) -> bool {
        self.deadline.eq(&other.deadline)
    }
}

impl Eq for ExpirationItem {}

impl PartialOrd for ExpirationItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExpirationItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering to make BinaryHeap a min-heap on deadline
        other.deadline.cmp(&self.deadline)
    }
}

fn apply_command(
    cmd: ExpirationCommand,
    heap: &mut BinaryHeap<ExpirationItem>,
    scheduled: &mut HashMap<u32, Instant>,
) {
    match cmd {
        ExpirationCommand::Schedule { id, deadline } => {
            // Keep the newest deadline and push to the heap for ordering
            scheduled.insert(id, deadline);
            heap.push(ExpirationItem { id, deadline });
        }
        ExpirationCommand::Cancel { id } => {
            // Cancel only updates the tracking map; stale heap entries are ignored
            scheduled.remove(&id);
        }
    }
}

fn maybe_compact(heap: &mut BinaryHeap<ExpirationItem>, scheduled: &HashMap<u32, Instant>) {
    // Count how many expiration entries are still real and expected to happen
    let live = scheduled.len();

    // If nothing is scheduled anymore, the heap has no useful work left to keep
    if live == 0 {
        heap.clear();
        return;
    }

    // Allow the heap to be bigger than the live set, but not wildly bigger
    let threshold = live.saturating_mul(4).max(128);

    // If the heap is still small enough, rebuilding it would just waste work
    if heap.len() <= threshold {
        return;
    }

    // Make a fresh heap sized for the entries that are still actually scheduled
    let mut rebuilt = BinaryHeap::with_capacity(live);

    // Copy each real scheduled expiration into the new clean heap
    for (id, deadline) in scheduled {
        rebuilt.push(ExpirationItem {
            id: *id,
            deadline: *deadline,
        });
    }

    // Swap out the old messy heap for the rebuilt one with only live entries
    *heap = rebuilt;
}

#[cfg(test)]
#[path = "expire/tests/mod.rs"]
mod tests;
