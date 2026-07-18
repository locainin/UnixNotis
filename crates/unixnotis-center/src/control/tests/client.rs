use super::UI_COMMAND_QUEUE_CAPACITY;

#[test]
fn command_queue_rejects_work_beyond_its_fixed_capacity() {
    let (sender, _receiver) = tokio::sync::mpsc::channel(UI_COMMAND_QUEUE_CAPACITY);
    for id in 0..UI_COMMAND_QUEUE_CAPACITY {
        sender
            .try_send(crate::control::UiCommand::Dismiss(
                u32::try_from(id).expect("test command id fits u32"),
            ))
            .expect("bounded queue accepts work below its limit");
    }

    assert!(matches!(
        sender.try_send(crate::control::UiCommand::Dismiss(u32::MAX)),
        Err(tokio::sync::mpsc::error::TrySendError::Full(_))
    ));
}
