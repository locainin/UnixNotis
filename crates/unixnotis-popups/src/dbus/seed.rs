//! Popup state seeding for one verified control owner

use std::fmt;

use unixnotis_core::{ControlState, NotificationView};

use super::types::UiEvent;

// Seed failures are returned to the owner state machine
#[derive(Debug)]
pub struct SeedError {
    state_error: Option<String>,
    active_error: Option<String>,
    send_error: Option<String>,
}

impl fmt::Display for SeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Each available stage preserves the bounded call or channel failure
        let failures = [
            self.state_error
                .as_deref()
                .map(|error| format!("GetState: {error}")),
            self.active_error
                .as_deref()
                .map(|error| format!("ListActive: {error}")),
            self.send_error
                .as_deref()
                .map(|error| format!("UI delivery: {error}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        write!(formatter, "{}", failures.join("; "))
    }
}

impl std::error::Error for SeedError {}

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

pub async fn seed_state<S>(
    proxy: &S,
    sender: &async_channel::Sender<UiEvent>,
) -> Result<(), SeedError>
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

#[cfg(test)]
#[path = "tests/seed.rs"]
mod tests;
