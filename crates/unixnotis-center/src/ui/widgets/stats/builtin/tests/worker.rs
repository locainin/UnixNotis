//! Built-in statistic worker tests

use super::{BuiltinJob, BuiltinSample, BuiltinWorker, SubmitOutcome};
use crate::ui::widgets::stats::builtin::BuiltinStat;

#[test]
fn builtin_worker_reports_a_full_queue_without_blocking() {
    let (tx, _worker_rx) = crossbeam_channel::bounded(1);
    let worker = BuiltinWorker {
        tx,
        inline_fallback: false,
    };
    let first = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let second = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let (first_tx, _first_rx) = async_channel::bounded(1);
    let (second_tx, _second_rx) = async_channel::bounded(1);

    assert_eq!(
        worker.submit(BuiltinJob {
            stat: first,
            respond: first_tx,
        }),
        SubmitOutcome::Submitted
    );
    assert_eq!(
        worker.submit(BuiltinJob {
            stat: second,
            respond: second_tx,
        }),
        SubmitOutcome::QueueFull
    );
}

#[test]
fn builtin_sample_preserves_reader_failure_as_missing_data() {
    let stat =
        BuiltinStat::from_command("builtin:net:unixnotis-missing-interface").expect("builtin stat");

    let sample = BuiltinSample::read(stat);

    assert!(sample.value.is_none());
}
