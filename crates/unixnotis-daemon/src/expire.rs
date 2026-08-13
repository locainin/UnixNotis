//! Notification expiration scheduling and timeouts

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tracing::warn;

use crate::daemon::DaemonState;
use crate::store::ExpirationTicket;
use unixnotis_core::CloseReason;

/// Commands sent to the expiration scheduler
pub enum ExpirationCommand {
    Schedule { ticket: ExpirationTicket },
    Cancel { id: u32, generation: u64 },
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
            let mut scheduled: HashMap<u32, ExpirationTicket> = HashMap::new();
            loop {
                let next_deadline = heap.peek().map(|item| item.ticket.deadline);
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
                    () = tokio::time::sleep_until(deadline.into()) => {
                        let now = Instant::now();
                        while let Some(item) = heap.peek() {
                            if item.ticket.deadline > now {
                                break;
                            }
                            let Some(item) = heap.pop() else {
                                break;
                            };
                            let is_current =
                                scheduled.get(&item.ticket.id) == Some(&item.ticket);
                            if !is_current {
                                continue;
                            }
                            // Validation and removal share the same store lock
                            let removed = {
                                let mut store = state.store.lock().await;
                                store.expire_if_current(item.ticket)
                            };
                            // Remove only the exact scheduler generation that was inspected
                            if scheduled.get(&item.ticket.id) == Some(&item.ticket) {
                                scheduled.remove(&item.ticket.id);
                            }
                            if removed.is_some() {
                                // Fanout happens only after the exact generation was removed
                                if let Err(err) = state
                                    .publish_notification_closed(
                                        unixnotis_core::NotificationKey {
                                            id: item.ticket.id,
                                            generation: item.ticket.generation,
                                        },
                                        CloseReason::Expired,
                                    )
                                    .await
                                {
                                    warn!(
                                        ?err,
                                        id = item.ticket.id,
                                        generation = item.ticket.generation,
                                        "failed to close expired notification"
                                    );
                                }
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

    pub fn schedule(&self, id: u32, generation: u64, deadline: Option<Instant>) {
        let command = match deadline {
            Some(deadline) => ExpirationCommand::Schedule {
                ticket: ExpirationTicket {
                    id,
                    generation,
                    deadline,
                },
            },
            None => ExpirationCommand::Cancel { id, generation },
        };
        if let Err(err) = self.sender.send(command) {
            warn!(?err, "expiration schedule request dropped");
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct ExpirationItem {
    ticket: ExpirationTicket,
}

impl PartialEq for ExpirationItem {
    fn eq(&self, other: &Self) -> bool {
        self.ticket.eq(&other.ticket)
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
        // Reverse every field so BinaryHeap remains a deterministic min-heap
        other
            .ticket
            .deadline
            .cmp(&self.ticket.deadline)
            .then_with(|| other.ticket.generation.cmp(&self.ticket.generation))
            .then_with(|| other.ticket.id.cmp(&self.ticket.id))
    }
}

fn apply_command(
    cmd: ExpirationCommand,
    heap: &mut BinaryHeap<ExpirationItem>,
    scheduled: &mut HashMap<u32, ExpirationTicket>,
) {
    match cmd {
        ExpirationCommand::Schedule { ticket } => {
            // Older commands cannot replace a later committed generation
            let may_replace = scheduled
                .get(&ticket.id)
                .is_none_or(|current| current.generation <= ticket.generation);
            if may_replace {
                scheduled.insert(ticket.id, ticket);
                heap.push(ExpirationItem { ticket });
            }
        }
        ExpirationCommand::Cancel { id, generation } => {
            // A delayed close from an older generation must preserve a replacement timer
            let may_remove = scheduled
                .get(&id)
                .is_some_and(|current| current.generation <= generation);
            if may_remove {
                scheduled.remove(&id);
            }
        }
    }
}

fn maybe_compact(
    heap: &mut BinaryHeap<ExpirationItem>,
    scheduled: &HashMap<u32, ExpirationTicket>,
) {
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
    for ticket in scheduled.values() {
        rebuilt.push(ExpirationItem { ticket: *ticket });
    }

    // Swap out the old messy heap for the rebuilt one with only live entries
    *heap = rebuilt;
}

#[cfg(test)]
#[path = "tests/expire.rs"]
mod tests;
