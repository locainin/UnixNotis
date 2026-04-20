//! Stat worker tests

use super::{
    stats_builtin::BuiltinStatKey, BuiltinStat, BuiltinStatJob, BuiltinStatWorker,
    BuiltinSubmitOutcome,
};

#[test]
fn builtin_worker_queue_full_falls_back() {
    let worker = BuiltinStatWorker::new_for_tests(1);
    let stat_a = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let stat_b = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let (tx_a, _rx_a) = async_channel::bounded(1);
    let (tx_b, _rx_b) = async_channel::bounded(1);

    // First job fits in the bounded queue
    assert_eq!(
        worker.submit(BuiltinStatJob {
            stat: stat_a,
            respond: tx_a,
        }),
        BuiltinSubmitOutcome::Submitted
    );
    // Second job proves the submit path reports saturation instead of blocking
    assert_eq!(
        worker.submit(BuiltinStatJob {
            stat: stat_b,
            respond: tx_b,
        }),
        BuiltinSubmitOutcome::QueueFull
    );
}

#[test]
fn builtin_stat_keys_dedupe_matching_sources() {
    let cpu_a = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let cpu_b = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let net = BuiltinStat::from_command("builtin:net:wlan0").expect("builtin stat");

    assert_eq!(cpu_a.key(), BuiltinStatKey::Cpu);
    assert_eq!(cpu_a.key(), cpu_b.key());
    assert_eq!(
        net.key(),
        BuiltinStatKey::Network {
            iface: Some("wlan0".to_string()),
        }
    );
}
