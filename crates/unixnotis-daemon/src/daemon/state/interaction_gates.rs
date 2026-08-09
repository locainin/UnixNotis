//! Bounded serialization for same-ID notification interactions

use tokio::sync::{Mutex, MutexGuard};

const INTERACTION_GATE_SHARDS: usize = 128;

/// Fixed interaction locks prevent an attacker-controlled notification ID space from growing state
pub(in crate::daemon) struct InteractionGates {
    shards: Box<[Mutex<()>]>,
}

impl InteractionGates {
    pub(in crate::daemon) fn new() -> Self {
        let shards = (0..INTERACTION_GATE_SHARDS)
            .map(|_index| Mutex::new(()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { shards }
    }

    pub(in crate::daemon) async fn lock(&self, id: u32) -> MutexGuard<'_, ()> {
        // IDs sharing a shard serialize conservatively while memory remains strictly bounded
        let index = interaction_gate_index(id);
        self.shards[index].lock().await
    }
}

fn interaction_gate_index(id: u32) -> usize {
    usize::try_from(id).unwrap_or(usize::MAX) % INTERACTION_GATE_SHARDS
}

#[cfg(test)]
#[path = "tests/interaction_gates.rs"]
mod tests;
