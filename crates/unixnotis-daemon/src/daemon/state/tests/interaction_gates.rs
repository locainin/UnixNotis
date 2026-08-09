use std::sync::Arc;

use super::{interaction_gate_index, InteractionGates, INTERACTION_GATE_SHARDS};

#[test]
fn every_protocol_id_maps_inside_the_fixed_interaction_shards() {
    assert_eq!(interaction_gate_index(0), 0);
    assert_eq!(interaction_gate_index(127), 127);
    assert_eq!(interaction_gate_index(128), 0);
    assert!(interaction_gate_index(u32::MAX) < INTERACTION_GATE_SHARDS);
}

#[tokio::test]
async fn same_id_waits_for_the_existing_interaction_guard() {
    let gates = Arc::new(InteractionGates::new());
    let first = gates.lock(42).await;
    let waiting_gates = Arc::clone(&gates);
    let waiting = tokio::spawn(async move {
        let _second = waiting_gates.lock(42).await;
    });

    tokio::task::yield_now().await;
    assert!(
        !waiting.is_finished(),
        "same-ID work must remain serialized"
    );
    drop(first);
    waiting.await.expect("waiting interaction task");
}

#[tokio::test]
async fn different_shards_can_progress_independently() {
    let gates = Arc::new(InteractionGates::new());
    let _first = gates.lock(1).await;
    let other_gates = Arc::clone(&gates);
    let other = tokio::spawn(async move {
        let _second = other_gates.lock(2).await;
    });

    tokio::time::timeout(std::time::Duration::from_millis(100), other)
        .await
        .expect("different shard should not wait")
        .expect("different-shard interaction task");
}
