use std::path::PathBuf;

use crossbeam_channel as channel;

use super::super::model::{IconSubmitError, IconUpdate};
use super::super::worker::{IconJob, IconWorker};
use crate::ui::icons::cache::IconKey;

fn key(name: &str) -> IconKey {
    IconKey::Path {
        path: PathBuf::from(name),
        size: 16,
        scale: 1,
    }
}

#[test]
fn icon_worker_queue_overflow_reports_failure() {
    let (update_tx, update_rx) = async_channel::bounded::<IconUpdate>(2);
    let (sender, _worker_rx) = channel::bounded::<IconJob>(1);
    let worker = IconWorker { sender };
    let _update_guard = update_tx;

    assert!(worker
        .submit_decode(key("icon-a.png"), PathBuf::from("icon-a.png"), 16, 1)
        .is_ok());
    let error = worker
        .submit_decode(key("icon-b.png"), PathBuf::from("icon-b.png"), 16, 1)
        .expect_err("queue should be full");

    assert!(matches!(error, IconSubmitError::Full));
    assert!(matches!(
        update_rx.try_recv(),
        Err(async_channel::TryRecvError::Empty)
    ));
}

#[test]
fn icon_submit_error_reasons_are_stable() {
    assert_eq!(
        IconSubmitError::Full.reason(),
        "icon decode queue full (drop-newest)"
    );
    assert_eq!(IconSubmitError::Closed.reason(), "icon decode queue closed");
}
