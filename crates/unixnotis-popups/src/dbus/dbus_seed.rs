//! Popup state seeding helpers

use std::time::{Duration, Instant};

use tracing::{debug, warn};
use unixnotis_core::ControlProxy;

use super::dbus_backoff::{Backoff, RetryLog};
use super::dbus_types::UiEvent;

// Seed retries tolerate short startup hiccups without blocking indefinitely
const SEED_RETRY_BASE_MS: u64 = 250;
const SEED_RETRY_MAX_MS: u64 = 2000;
const SEED_RETRY_BUDGET_SECS: u64 = 30;
const SEED_RETRY_LOG_INTERVAL_SECS: u64 = 10;

// Seed failures are tracked without forcing an immediate reconnect
#[derive(Debug)]
struct SeedError {
    state_error: Option<String>,
    active_error: Option<String>,
    send_error: Option<String>,
}

pub(crate) async fn seed_state_with_retry(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
) {
    // Seed retries stay bounded so startup can recover without hanging forever
    let mut backoff = Backoff::new(SEED_RETRY_BASE_MS, SEED_RETRY_MAX_MS);
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

async fn seed_state(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
) -> Result<(), SeedError> {
    // Best-effort seeding still uses two daemon RPCs, so fetch both together to shrink skew
    // A fully atomic seed would need one daemon method that returns both pieces at once
    let (state, active) = tokio::join!(proxy.get_state(), proxy.list_active());

    match (state, active) {
        (Ok(state), Ok(active)) => send_seed_event(sender, UiEvent::Seed { state, active }).await,
        (state, active) => Err(SeedError {
            state_error: state.err().map(|err| err.to_string()),
            active_error: active.err().map(|err| err.to_string()),
            send_error: None,
        }),
    }
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

fn log_seed_retry(log: &mut RetryLog, err: &SeedError, message: &str) {
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
    );
}

#[cfg(test)]
mod tests {
    use async_channel::bounded;
    use unixnotis_core::ControlState;

    use crate::dbus::UiEvent;

    use super::send_seed_event;

    #[tokio::test]
    async fn closed_seed_channel_returns_error() {
        let (tx, rx) = bounded(1);
        drop(rx);

        let err = send_seed_event(
            &tx,
            UiEvent::Seed {
                state: ControlState::default(),
                active: Vec::new(),
            },
        )
        .await
        .expect_err("closed seed channel should fail");

        assert!(err.send_error.is_some());
    }
}
