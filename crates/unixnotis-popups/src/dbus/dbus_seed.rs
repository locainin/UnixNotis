//! Popup state seeding helpers

use std::time::{Duration, Instant};

use tracing::{debug, warn};
use unixnotis_core::{ControlState, NotificationView};

use super::dbus_backoff::{Backoff, RetryLog};
use super::dbus_types::UiEvent;

// Seed retries tolerate short startup hiccups without blocking indefinitely
const SEED_RETRY_BASE_MS: u64 = 250;
const SEED_RETRY_MAX_MS: u64 = 2000;
const SEED_RETRY_BUDGET_SECS: u64 = 30;
const SEED_RETRY_LOG_INTERVAL_SECS: u64 = 10;

// Seed failures are tracked without forcing an immediate reconnect
#[derive(Debug)]
pub struct SeedError {
    state_error: Option<String>,
    active_error: Option<String>,
    send_error: Option<String>,
}

#[derive(Debug)]
pub struct SeedSnapshot {
    // State and active rows are sent together so reconnect seeding cannot mix old and new data
    state: ControlState,
    active: Vec<NotificationView>,
}

impl SeedSnapshot {
    pub(crate) fn from_fetch_results(
        state: zbus::Result<ControlState>,
        active: zbus::Result<Vec<NotificationView>>,
    ) -> Result<Self, SeedError> {
        // Convert both RPC results in one place so retry tests cover each failure shape
        match (state, active) {
            (Ok(state), Ok(active)) => Ok(Self { state, active }),
            (state, active) => Err(SeedError {
                state_error: state.err().map(|err| err.to_string()),
                active_error: active.err().map(|err| err.to_string()),
                send_error: None,
            }),
        }
    }
}

pub trait PopupSeedSource {
    async fn seed_snapshot(&self) -> Result<SeedSnapshot, SeedError>;
}

pub async fn seed_state_with_retry<S>(proxy: &S, sender: &async_channel::Sender<UiEvent>)
where
    S: PopupSeedSource,
{
    // Seed retries stay bounded so startup can recover without hanging forever
    let deadline = seed_retry_deadline(Instant::now());
    seed_state_with_retry_until(proxy, sender, deadline).await;
}

async fn seed_state_with_retry_until<S>(
    proxy: &S,
    sender: &async_channel::Sender<UiEvent>,
    deadline: Instant,
) where
    S: PopupSeedSource,
{
    // Seed retries stay bounded so startup can recover without hanging forever
    let mut backoff = Backoff::new(SEED_RETRY_BASE_MS, SEED_RETRY_MAX_MS);
    let mut log = RetryLog::new(Duration::from_secs(SEED_RETRY_LOG_INTERVAL_SECS));

    loop {
        match seed_state(proxy, sender).await {
            Ok(()) => return,
            Err(err) => {
                if Instant::now() >= deadline {
                    warn!(
                        state_error = ?err.state_error,
                        active_error = ?err.active_error,
                        "failed to seed popup state; giving up until reconnect"
                    );
                    return;
                }
                log_seed_retry(&mut log, &err, "failed to seed popup state; retrying");
                tokio::time::sleep(backoff.next_sleep()).await;
            }
        }
    }
}

fn seed_retry_deadline(now: Instant) -> Instant {
    now + Duration::from_secs(SEED_RETRY_BUDGET_SECS)
}

async fn seed_state<S>(proxy: &S, sender: &async_channel::Sender<UiEvent>) -> Result<(), SeedError>
where
    S: PopupSeedSource,
{
    let snapshot = proxy.seed_snapshot().await?;
    send_seed_event(
        sender,
        UiEvent::Seed {
            state: snapshot.state,
            active: snapshot.active,
        },
    )
    .await
}

async fn send_seed_event(
    sender: &async_channel::Sender<UiEvent>,
    event: UiEvent,
) -> Result<(), SeedError> {
    // Closed receiver means startup never applied the seed, so this must stay retryable
    sender.send(event).await.map_err(|err| SeedError {
        state_error: None,
        active_error: None,
        // Closed channel means the seed never reached the UI, so retry state should stay failed
        send_error: Some(err.to_string()),
    })
}

fn log_seed_retry(log: &mut RetryLog, err: &SeedError, message: &str) -> bool {
    log.log_with(
        || {
            warn!(
                state_error = ?err.state_error,
                active_error = ?err.active_error,
                send_error = ?err.send_error,
                "{message}"
            );
        },
        || {
            debug!(
                state_error = ?err.state_error,
                active_error = ?err.active_error,
                send_error = ?err.send_error,
                "{message}"
            );
        },
    )
}

#[cfg(test)]
#[path = "tests/seed.rs"]
mod tests;
