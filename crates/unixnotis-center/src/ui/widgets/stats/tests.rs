//! Stat worker tests

use super::{BuiltinStat, BuiltinStatJob, BuiltinStatWorker};

#[test]
fn builtin_worker_queue_full_falls_back() {
    let worker = BuiltinStatWorker::new_for_tests(1);
    let stat_a = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let stat_b = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let (tx_a, _rx_a) = async_channel::bounded(1);
    let (tx_b, _rx_b) = async_channel::bounded(1);

    // First job fits in the bounded queue
    assert!(worker.submit(BuiltinStatJob {
        stat: stat_a,
        respond: tx_a,
    }));
    // Second job proves the submit path reports saturation instead of blocking
    assert!(!worker.submit(BuiltinStatJob {
        stat: stat_b,
        respond: tx_b,
    }));
}
