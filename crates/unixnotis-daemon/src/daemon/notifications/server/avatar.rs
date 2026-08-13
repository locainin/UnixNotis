//! Bounded worker support for sender-provided conversation artwork

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

const AVATAR_WORKER_SLOTS: usize = 4;

fn avatar_worker_pool() -> Arc<Semaphore> {
    static POOL: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();
    Arc::clone(POOL.get_or_init(|| Arc::new(Semaphore::new(AVATAR_WORKER_SLOTS))))
}

pub(super) async fn run_avatar_worker<T, F>(work: F, deadline: Duration) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    run_avatar_worker_with_pool(avatar_worker_pool(), work, deadline).await
}

pub(super) async fn run_avatar_worker_with_pool<T, F>(
    pool: Arc<Semaphore>,
    work: F,
    deadline: Duration,
) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    // try_acquire makes overload fail closed instead of queuing unbounded work
    let permit = pool.try_acquire_owned().ok()?;
    let task = tokio::task::spawn_blocking(move || {
        // Keep this permit in the blocking closure so timeout cancellation cannot release it early
        let _permit = permit;
        work()
    });
    tokio::time::timeout(deadline, task)
        .await
        .ok()
        .and_then(Result::ok)
}

#[cfg(test)]
#[path = "tests/avatar.rs"]
mod tests;
