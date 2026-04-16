//! Seeding helpers for initial control state sync over D-Bus.

use std::time::{Duration, Instant};

use tokio::time::sleep;
use tracing::{debug, warn};
use unixnotis_core::ControlProxy;

use super::dbus_backoff::RetryLog;
use super::dbus_types::UiEvent;

// Seed retries tolerate short startup hiccups without blocking indefinitely.
pub(crate) const SEED_RETRY_BASE_MS: u64 = 250;
pub(crate) const SEED_RETRY_MAX_MS: u64 = 2000;
pub(crate) const SEED_RETRY_BUDGET_SECS: u64 = 30;
pub(crate) const SEED_RETRY_LOG_INTERVAL_SECS: u64 = 10;

// Captures seed failures without forcing an immediate reconnect.
#[derive(Debug)]
pub(crate) struct SeedError {
    pub(crate) state_error: Option<String>,
    pub(crate) active_error: Option<String>,
    pub(crate) history_error: Option<String>,
}

pub(crate) async fn seed_state_with_retry(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
) {
    // Seed retries are bounded to keep startup responsive while tolerating transient failures.
    let mut backoff = super::dbus_backoff::Backoff::new(SEED_RETRY_BASE_MS, SEED_RETRY_MAX_MS);
    let deadline = Instant::now() + Duration::from_secs(SEED_RETRY_BUDGET_SECS);
    let mut log = RetryLog::new(Duration::from_secs(SEED_RETRY_LOG_INTERVAL_SECS));

    loop {
        match seed_state(proxy, sender).await {
            Ok(()) => return,
            Err(err) => {
                if Instant::now() >= deadline {
                    warn!(
                        state_error = ?err.state_error,
                        active_error = ?err.active_error,
                        history_error = ?err.history_error,
                        "failed to seed center state; giving up until reconnect"
                    );
                    return;
                }
                log.log_with(
                    || {
                        warn!(
                            state_error = ?err.state_error,
                            active_error = ?err.active_error,
                            history_error = ?err.history_error,
                            "failed to seed center state; retrying"
                        );
                    },
                    || {
                        debug!(
                            state_error = ?err.state_error,
                            active_error = ?err.active_error,
                            history_error = ?err.history_error,
                            "failed to seed center state; retrying"
                        );
                    },
                );
                sleep(backoff.next_sleep()).await;
            }
        }
    }
}

pub(crate) async fn seed_state(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
) -> Result<(), SeedError> {
    // Fetch in parallel so startup waits on the slowest call instead of the sum of all calls.
    let (state, active, history) =
        tokio::join!(proxy.get_state(), proxy.list_active(), proxy.list_history());

    match (state, active, history) {
        (Ok(state), Ok(active), Ok(history)) => {
            let _ = sender
                .send(UiEvent::Seed {
                    state,
                    active,
                    history,
                })
                .await;
            Ok(())
        }
        (state, active, history) => Err(SeedError {
            state_error: state.err().map(|err| err.to_string()),
            active_error: active.err().map(|err| err.to_string()),
            history_error: history.err().map(|err| err.to_string()),
        }),
    }
}
