use async_channel::bounded;
use unixnotis_core::ControlState;

use super::{seed_state, send_seed_event, PopupSeedSource, SeedError, SeedSnapshot};
use crate::dbus::UiEvent;

struct FakeSeedSource {
    state_ok: bool,
    active_ok: bool,
}

impl FakeSeedSource {
    fn available() -> Self {
        Self {
            state_ok: true,
            active_ok: true,
        }
    }

    fn missing_state() -> Self {
        Self {
            state_ok: false,
            active_ok: true,
        }
    }

    fn missing_active() -> Self {
        Self {
            state_ok: true,
            active_ok: false,
        }
    }
}

impl PopupSeedSource for FakeSeedSource {
    async fn seed_snapshot(&self) -> Result<SeedSnapshot, SeedError> {
        let state = if self.state_ok {
            Ok(ControlState::default())
        } else {
            Err(zbus::Error::Failure("state unavailable".to_string()))
        };
        let active = if self.active_ok {
            Ok(Vec::new())
        } else {
            Err(zbus::Error::Failure("active unavailable".to_string()))
        };

        SeedSnapshot::from_fetch_results(state, active)
    }
}

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

#[tokio::test]
async fn seed_state_sends_seed_event_when_fetches_succeed() {
    let source = FakeSeedSource::available();
    let (tx, rx) = bounded(1);

    seed_state(&source, &tx)
        .await
        .expect("seed state should send");

    let event = rx.try_recv().expect("seed event should be queued");
    assert!(matches!(event, UiEvent::Seed { .. }));
}

#[tokio::test]
async fn seed_state_reports_state_fetch_failure_without_sending() {
    let source = FakeSeedSource::missing_state();
    let (tx, rx) = bounded(1);

    let err = seed_state(&source, &tx)
        .await
        .expect_err("state failure should be returned");

    assert!(err.state_error.is_some());
    assert!(err.active_error.is_none());
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn seed_state_reports_active_fetch_failure_without_sending() {
    let source = FakeSeedSource::missing_active();
    let (tx, rx) = bounded(1);

    let err = seed_state(&source, &tx)
        .await
        .expect_err("active failure should be returned");

    assert!(err.state_error.is_none());
    assert!(err.active_error.is_some());
    assert!(rx.try_recv().is_err());
}
