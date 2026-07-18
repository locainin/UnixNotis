use tokio::sync::mpsc;

use super::super::{MediaCommand, MediaHandle};

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
