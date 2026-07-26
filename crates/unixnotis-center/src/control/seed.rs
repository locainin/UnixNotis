//! Bounded seeding helpers for one verified control owner

use unixnotis_core::{timed_dbus_call, ControlProxy};

use super::model::UiEvent;

// Each error identifies the exact stage that prevented one complete snapshot
#[derive(Debug)]
pub struct SeedError {
    pub(crate) state_error: Option<String>,
    pub(crate) active_error: Option<String>,
    pub(crate) history_error: Option<String>,
    pub(crate) send_error: Option<String>,
}

#[cfg(test)]
#[path = "tests/seed.rs"]
mod tests;

pub async fn seed_state(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
) -> Result<(), SeedError> {
    // GetState is the handshake and must succeed before snapshot methods are issued
    let state = timed_dbus_call(proxy.get_state())
        .await
        .map_err(|error| SeedError {
            state_error: Some(error.to_string()),
            active_error: None,
            history_error: None,
            send_error: None,
        })?;
    let (active, history) = tokio::join!(
        timed_dbus_call(proxy.list_active()),
        timed_dbus_call(proxy.list_history())
    );

    match (active, history) {
        (Ok(active), Ok(history)) => {
            // Publish only complete snapshots so the UI never mixes generations
            sender
                .send(UiEvent::Seed {
                    state,
                    active,
                    history,
                })
                .await
                .map_err(|error| SeedError {
                    state_error: None,
                    active_error: None,
                    history_error: None,
                    send_error: Some(error.to_string()),
                })?;
            Ok(())
        }
        // Individual errors remain separate for useful diagnostics
        (active, history) => Err(SeedError {
            state_error: None,
            active_error: active.err().map(|err| err.to_string()),
            history_error: history.err().map(|err| err.to_string()),
            send_error: None,
        }),
    }
}
