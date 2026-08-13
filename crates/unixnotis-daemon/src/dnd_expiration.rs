//! Single-deadline scheduler for timed Do Not Disturb state

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tracing::warn;

use crate::daemon::DaemonState;

const MAX_CLOCK_RECHECK: Duration = Duration::from_mins(1);
const PERSIST_RETRY_DELAY: Duration = Duration::from_secs(5);

/// Coalescing scheduler handle for the one active DND deadline
#[derive(Clone)]
pub struct DndExpirationScheduler {
    sender: watch::Sender<Option<i64>>,
}

impl DndExpirationScheduler {
    pub fn start(state: Arc<DaemonState>) -> Self {
        // A watch channel keeps only the newest deadline during rapid menu changes
        let (sender, mut receiver) = watch::channel(None);
        tokio::spawn(async move {
            loop {
                let expires_at = *receiver.borrow_and_update();
                let Some(expires_at) = expires_at else {
                    // No deadline means indefinite or disabled DND
                    if receiver.changed().await.is_err() {
                        break;
                    }
                    continue;
                };

                let delay = delay_until_recheck(chrono::Utc::now().timestamp(), expires_at);
                if delay.is_zero() {
                    // The store verifies this is still the current deadline before mutating
                    if let Err(err) = state.apply_dnd_expiration(expires_at).await {
                        warn!(
                            ?err,
                            expires_at, "failed to expire timed do-not-disturb state"
                        );
                        // A persistence outage must not create a tight retry loop
                        tokio::time::sleep(PERSIST_RETRY_DELAY).await;
                    }
                    continue;
                }

                tokio::select! {
                    changed = receiver.changed() => {
                        if changed.is_err() {
                            break;
                        }
                    }
                    () = tokio::time::sleep(delay) => {
                        // Wall time is checked again so clock adjustments cannot skip expiry
                    }
                }
            }
        });

        Self { sender }
    }

    pub fn schedule(&self, expires_at: Option<i64>) {
        // Replacing the watch value cancels the previous logical deadline
        self.sender.send_replace(expires_at);
    }
}

fn delay_until_recheck(now: i64, expires_at: i64) -> Duration {
    let remaining = expires_at.saturating_sub(now);
    if remaining <= 0 {
        return Duration::ZERO;
    }
    Duration::from_secs(remaining as u64).min(MAX_CLOCK_RECHECK)
}

#[cfg(test)]
#[path = "tests/dnd_expiration.rs"]
mod tests;
