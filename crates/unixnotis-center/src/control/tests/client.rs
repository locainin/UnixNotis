use super::UI_COMMAND_QUEUE_CAPACITY;
use unixnotis_core::NotificationKey;

#[test]
fn command_queue_rejects_work_beyond_its_fixed_capacity() {
    let (sender, _receiver) = tokio::sync::mpsc::channel(UI_COMMAND_QUEUE_CAPACITY);
    for id in 0..UI_COMMAND_QUEUE_CAPACITY {
        sender
            .try_send(crate::control::UiCommand::Dismiss(NotificationKey {
                id: u32::try_from(id).expect("test command id fits u32"),
                generation: u64::try_from(id).expect("test generation fits u64"),
            }))
            .expect("bounded queue accepts work below its limit");
    }

    assert!(matches!(
        sender.try_send(crate::control::UiCommand::Dismiss(NotificationKey {
            id: u32::MAX,
            generation: u64::MAX,
        })),
        Err(tokio::sync::mpsc::error::TrySendError::Full(_))
    ));
}
