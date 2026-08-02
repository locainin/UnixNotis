//! Tests for bounded conversation-avatar work

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use tokio::sync::Semaphore;

use super::super::avatar::run_avatar_worker_with_pool;

#[tokio::test]
async fn avatar_worker_capacity_fails_closed_without_queueing() {
    let pool = Arc::new(Semaphore::new(1));
    let release = Arc::new(AtomicBool::new(false));
    let held_release = Arc::clone(&release);

    let first = run_avatar_worker_with_pool(
        Arc::clone(&pool),
        move || {
            while !held_release.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            1_u8
        },
        Duration::from_millis(10),
    );
    assert_eq!(first.await, None);

    let second =
        run_avatar_worker_with_pool(Arc::clone(&pool), || 2_u8, Duration::from_millis(10)).await;
    assert_eq!(second, None);

    release.store(true, Ordering::Release);
    tokio::time::sleep(Duration::from_millis(20)).await;

    let recovered = run_avatar_worker_with_pool(pool, || 3_u8, Duration::from_millis(100)).await;
    assert_eq!(recovered, Some(3));
}
