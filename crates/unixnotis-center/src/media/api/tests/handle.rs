use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;

use super::super::{start_media_task, MediaCommand, MediaHandle};

#[test]
fn disabled_media_does_not_start_a_runtime_task() {
    let runtime = tokio::runtime::Runtime::new().expect("create test runtime");
    let (event_tx, _event_rx) = async_channel::bounded(1);
    let config = MediaConfig {
        enabled: false,
        ..MediaConfig::default()
    };

    let handle = start_media_task(runtime.handle(), config, event_tx);

    assert!(handle.is_none());
}

#[test]
fn enabled_media_returns_a_connected_command_handle() {
    let runtime = tokio::runtime::Runtime::new().expect("create test runtime");
    let (event_tx, _event_rx) = async_channel::bounded(1);

    let handle = start_media_task(runtime.handle(), MediaConfig::default(), event_tx);

    assert!(handle.is_some());
}

#[test]
fn media_controls_send_the_requested_player_commands() {
    let runtime = tokio::runtime::Runtime::new().expect("create test runtime");
    let (sender, mut receiver) = mpsc::channel(4);
    let handle = MediaHandle::connected(sender, runtime.handle().clone());

    handle.next("org.mpris.MediaPlayer2.test");
    handle.previous("org.mpris.MediaPlayer2.test");
    handle.play_pause("org.mpris.MediaPlayer2.test");
    handle.refresh();

    assert!(matches!(receiver.try_recv(), Ok(MediaCommand::Next { .. })));
    assert!(matches!(
        receiver.try_recv(),
        Ok(MediaCommand::Previous { .. })
    ));
    assert!(matches!(
        receiver.try_recv(),
        Ok(MediaCommand::PlayPause { .. })
    ));
    assert!(matches!(receiver.try_recv(), Ok(MediaCommand::Refresh)));
}

#[test]
fn disconnected_handle_accepts_controls_without_queueing_work() {
    let runtime = tokio::runtime::Runtime::new().expect("create test runtime");
    let (sender, receiver) = mpsc::channel(1);
    drop(receiver);
    let handle = MediaHandle::connected(sender, runtime.handle().clone());

    handle.next("org.mpris.MediaPlayer2.missing");
    handle.refresh();
}
