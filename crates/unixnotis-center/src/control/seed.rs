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
    // The daemon captures state and rows under one store lock
    match timed_dbus_call(proxy.get_snapshot()).await {
        Ok(snapshot) => {
            // Publish only complete snapshots so the UI never mixes generations
            sender
                .send(UiEvent::Seed {
                    state: snapshot.state,
                    active: snapshot.active,
                    history: snapshot.history,
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
        Err(error) => Err(SeedError {
            state_error: Some(error.to_string()),
            active_error: None,
            history_error: None,
            send_error: None,
        }),
    }
}
