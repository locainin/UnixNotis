//! Monotonic fairness leases for full MPRIS inventories

use std::collections::HashSet;
use std::time::Duration;

use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::constants::{MPRIS_FAIRNESS_LEASE_MS, MPRIS_FAIRNESS_RETRY_MS};
use crate::media::runtime::MediaSignal;

pub(in crate::media) struct MprisFairnessState {
    deadline: Option<Instant>,
    wakeup: Option<(u64, JoinHandle<()>)>,
    generation: u64,
    victim_cursor: usize,
    lease_duration: Duration,
    retry_duration: Duration,
}

impl MprisFairnessState {
    pub(in crate::media) const fn new() -> Self {
        Self::with_durations(
            Duration::from_millis(MPRIS_FAIRNESS_LEASE_MS),
            Duration::from_millis(MPRIS_FAIRNESS_RETRY_MS),
        )
    }

    pub(in crate::media) const fn with_durations(
        lease_duration: Duration,
        retry_duration: Duration,
    ) -> Self {
        Self {
            deadline: None,
            wakeup: None,
            generation: 0,
            victim_cursor: 0,
            lease_duration,
            retry_duration,
        }
    }

    pub(in crate::media) fn rotation_due(
        &mut self,
        capacity_was_full: bool,
        has_untracked: bool,
        now: Instant,
        signal_tx: &Sender<MediaSignal>,
    ) -> bool {
        if !capacity_was_full || !has_untracked {
            self.clear_lease();
            return false;
        }
        let Some(deadline) = self.deadline else {
            self.start_lease(now, signal_tx);
            return false;
        };
        if now >= deadline {
            return true;
        }
        self.ensure_wakeup(deadline, signal_tx);
        false
    }

    pub(in crate::media) fn complete_rotation(
        &mut self,
        now: Instant,
        has_untracked: bool,
        signal_tx: &Sender<MediaSignal>,
    ) {
        // Admission completion is the only event that renews an active fairness lease
        self.clear_lease();
        if has_untracked {
            self.start_lease(now, signal_tx);
        }
    }

    pub(in crate::media) fn retry_failed_rotation(
        &mut self,
        now: Instant,
        signal_tx: &Sender<MediaSignal>,
    ) {
        if self.deadline.is_some() && self.wakeup.is_none() {
            self.ensure_wakeup(now + self.retry_duration, signal_tx);
        }
    }

    pub(in crate::media) fn consume_wakeup(&mut self, generation: u64) -> bool {
        let matches_current = self
            .wakeup
            .as_ref()
            .is_some_and(|(scheduled_generation, _task)| *scheduled_generation == generation)
            && self.generation == generation
            && self.deadline.is_some();
        if matches_current {
            self.wakeup.take();
        }
        matches_current
    }

    pub(in crate::media) fn select_victim(&mut self, tracked: &HashSet<String>) -> Option<String> {
        let mut tracked = tracked.iter().collect::<Vec<_>>();
        tracked.sort_unstable();
        if tracked.is_empty() {
            return None;
        }
        let victim = (*tracked.get(self.victim_cursor % tracked.len())?).clone();
        self.victim_cursor = (self.victim_cursor + 1) % tracked.len();
        Some(victim)
    }

    fn start_lease(&mut self, now: Instant, signal_tx: &Sender<MediaSignal>) {
        self.generation = self.generation.wrapping_add(1);
        let deadline = now + self.lease_duration;
        self.deadline = Some(deadline);
        self.ensure_wakeup(deadline, signal_tx);
    }

    fn ensure_wakeup(&mut self, wake_at: Instant, signal_tx: &Sender<MediaSignal>) {
        if self.wakeup.is_some() {
            return;
        }
        let generation = self.generation;
        let signal_tx = signal_tx.clone();
        // Exactly one task converts monotonic lease time into an event-loop refresh
        let task = tokio::spawn(async move {
            tokio::time::sleep_until(wake_at).await;
            let _ = signal_tx
                .send(MediaSignal::FairnessLeaseExpired { generation })
                .await;
        });
        self.wakeup = Some((generation, task));
    }

    fn clear_lease(&mut self) {
        if let Some((_generation, task)) = self.wakeup.take() {
            task.abort();
        }
        if self.deadline.take().is_some() {
            // Queued messages from an old lease must not wake the renewed inventory
            self.generation = self.generation.wrapping_add(1);
        }
    }
}

impl Drop for MprisFairnessState {
    fn drop(&mut self) {
        if let Some((_generation, task)) = self.wakeup.take() {
            task.abort();
        }
    }
}
