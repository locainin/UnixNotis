//! Notification expiration scheduling and timeouts.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tracing::warn;

use crate::daemon::DaemonState;
use unixnotis_core::CloseReason;

/// Commands sent to the expiration scheduler.
pub enum ExpirationCommand {
    Schedule { id: u32, deadline: Instant },
    Cancel { id: u32 },
}

/// Asynchronous expiration manager backed by a priority queue.
#[derive(Clone)]
pub struct ExpirationScheduler {
    sender: mpsc::Sender<ExpirationCommand>,
}

const EXPIRATION_QUEUE_CAPACITY: usize = 256;

impl ExpirationScheduler {
    pub fn start(state: Arc<DaemonState>) -> Self {
        let (sender, mut receiver) = mpsc::channel(EXPIRATION_QUEUE_CAPACITY);
        tokio::spawn(async move {
            let mut heap: BinaryHeap<ExpirationItem> = BinaryHeap::new();
            // Tracks the latest deadline per notification to discard stale heap entries.
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
                            // Remove the scheduled entry once the matching deadline is handled.
                            scheduled.remove(&item.id);
                            // Verify the deadline is still current before closing the notification.
                            let should_expire = {
                                let store = state.store.lock().await;
                                store
                                    .expiration_for(item.id)
                                    .map(|deadline| deadline == item.deadline)
                                    .unwrap_or(false)
                            };
                            if should_expire {
                                let _ = state.close_notification(item.id, CloseReason::Expired).await;
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

    pub async fn schedule(&self, id: u32, deadline: Option<Instant>) {
        let command = match deadline {
            Some(deadline) => ExpirationCommand::Schedule { id, deadline },
            None => ExpirationCommand::Cancel { id },
        };
        if let Err(err) = self.sender.send(command).await {
            warn!(?err, "expiration schedule request dropped");
        }
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
        // Reverse ordering to make BinaryHeap a min-heap on deadline.
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
            // Keep the newest deadline and push to the heap for ordering.
            scheduled.insert(id, deadline);
            heap.push(ExpirationItem { id, deadline });
        }
        ExpirationCommand::Cancel { id } => {
            // Cancel only updates the tracking map; stale heap entries are ignored.
            scheduled.remove(&id);
        }
    }
}

fn maybe_compact(heap: &mut BinaryHeap<ExpirationItem>, scheduled: &HashMap<u32, Instant>) {
    let live = scheduled.len();
    if live == 0 {
        heap.clear();
        return;
    }
    let threshold = live.saturating_mul(4).max(128);
    if heap.len() <= threshold {
        return;
    }
    let mut rebuilt = BinaryHeap::with_capacity(live);
    for (id, deadline) in scheduled {
        rebuilt.push(ExpirationItem {
            id: *id,
            deadline: *deadline,
        });
    }
    *heap = rebuilt;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn expiration_heap_orders_by_deadline() {
        let now = Instant::now();
        let mut heap = BinaryHeap::new();
        heap.push(ExpirationItem {
            id: 1,
            deadline: now + Duration::from_secs(2),
        });
        heap.push(ExpirationItem {
            id: 2,
            deadline: now + Duration::from_secs(1),
        });

        let first = heap.pop().expect("first item");
        assert_eq!(first.id, 2);
    }

    #[test]
    fn apply_command_tracks_latest_schedule() {
        let now = Instant::now();
        let mut heap = BinaryHeap::new();
        let mut scheduled = HashMap::new();

        apply_command(
            ExpirationCommand::Schedule {
                id: 7,
                deadline: now + Duration::from_secs(5),
            },
            &mut heap,
            &mut scheduled,
        );
        apply_command(
            ExpirationCommand::Schedule {
                id: 7,
                deadline: now + Duration::from_secs(3),
            },
            &mut heap,
            &mut scheduled,
        );

        assert_eq!(scheduled.len(), 1);
        assert_eq!(scheduled.get(&7), Some(&(now + Duration::from_secs(3))));
        assert_eq!(heap.len(), 2);
    }

    #[test]
    fn apply_command_cancel_removes_schedule() {
        let now = Instant::now();
        let mut heap = BinaryHeap::new();
        let mut scheduled = HashMap::new();

        apply_command(
            ExpirationCommand::Schedule {
                id: 9,
                deadline: now + Duration::from_secs(2),
            },
            &mut heap,
            &mut scheduled,
        );
        apply_command(
            ExpirationCommand::Cancel { id: 9 },
            &mut heap,
            &mut scheduled,
        );

        assert!(scheduled.is_empty());
    }

    #[test]
    fn maybe_compact_rebuilds_from_scheduled() {
        let now = Instant::now();
        let mut heap = BinaryHeap::new();
        let mut scheduled = HashMap::new();

        scheduled.insert(1_u32, now + Duration::from_secs(1));
        for id in 0..129_u32 {
            heap.push(ExpirationItem {
                id,
                deadline: now + Duration::from_secs(id as u64 + 1),
            });
        }

        maybe_compact(&mut heap, &scheduled);
        assert_eq!(heap.len(), scheduled.len());
        let item = heap.pop().expect("rebuilt item");
        assert_eq!(item.id, 1);
    }
}
