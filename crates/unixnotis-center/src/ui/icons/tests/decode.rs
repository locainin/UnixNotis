use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn icon_worker_queue_overflow_reports_failure() {
    let (update_tx, update_rx) = async_channel::bounded(2);
    let worker = IconWorker::new_for_tests(update_tx, 1);
    let key_a = IconKey::Path {
        path: "icon-a.png".to_string(),
        size: 16,
        scale: 1,
    };
    let key_b = IconKey::Path {
        path: "icon-b.png".to_string(),
        size: 16,
        scale: 1,
    };

    assert!(worker
        .submit_decode(
            key_a,
            PathBuf::from("icon-a.png"),
            16,
            1,
            IconDecodeMode::Raster,
        )
        .is_ok());
    let err = worker
        .submit_decode(
            key_b,
            PathBuf::from("icon-b.png"),
            16,
            1,
            IconDecodeMode::Raster,
        )
        .expect_err("queue should be full");

    assert!(matches!(err, IconSubmitError::Full));
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

#[test]
fn decode_raster_reports_missing_file() {
    let result = decode_raster(Path::new("missing-icon.png"), 16, 1);
    assert!(matches!(result, IconResult::Failed(_)));
}

#[test]
fn load_bytes_reads_small_file() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("unixnotis-icon-bytes-{stamp}.bin"));
    let payload = b"svg-bytes";
    fs::write(&path, payload).expect("write temp icon bytes");

    let result = load_bytes(&path);
    match result {
        IconResult::Bytes(bytes) => assert_eq!(bytes, payload),
        other => panic!("unexpected result: {other:?}"),
    }

    let _ = fs::remove_file(&path);
}
