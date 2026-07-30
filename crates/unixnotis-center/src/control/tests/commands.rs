use super::*;
use unixnotis_core::NotificationKey;

#[test]
fn drop_stale_offline_commands_retains_safe_actions() {
    // Mix stale id-based actions with reconnect-safe commands
    let mut offline = VecDeque::new();
    offline.push_back(UiCommand::Dismiss(NotificationKey {
        id: 10,
        generation: 12,
    }));
    offline.push_back(UiCommand::InvokeAction {
        notification: NotificationKey {
            id: 11,
            generation: 13,
        },
        action_key: "open".to_string(),
        confirmed: false,
    });
    offline.push_back(UiCommand::SetDnd(true));
    offline.push_back(UiCommand::ClearAll);
    offline.push_back(UiCommand::ClosePanel);

    drop_stale_offline_commands(&mut offline);

    // Only commands that can survive reconnect without id drift should remain
    assert_eq!(offline.len(), 3);
    assert!(offline
        .iter()
        .any(|cmd| matches!(cmd, UiCommand::SetDnd(true))));
    assert!(offline.iter().any(|cmd| matches!(cmd, UiCommand::ClearAll)));
    assert!(offline
        .iter()
        .any(|cmd| matches!(cmd, UiCommand::ClosePanel)));
}

#[test]
fn enqueue_offline_command_drops_duplicate_one_shot_commands() {
    let mut offline = VecDeque::new();

    assert!(enqueue_offline_command(&mut offline, UiCommand::ClosePanel));
    assert!(!enqueue_offline_command(
        &mut offline,
        UiCommand::ClosePanel
    ));
    assert!(enqueue_offline_command(&mut offline, UiCommand::ClearAll));
    assert!(!enqueue_offline_command(&mut offline, UiCommand::ClearAll));

    assert_eq!(offline.len(), 2);
    assert!(matches!(offline[0], UiCommand::ClosePanel));
    assert!(matches!(offline[1], UiCommand::ClearAll));
}

#[test]
fn enqueue_offline_command_keeps_latest_dnd_state_only() {
    let mut offline = VecDeque::new();

    assert!(enqueue_offline_command(
        &mut offline,
        UiCommand::SetDnd(true)
    ));
    assert!(enqueue_offline_command(
        &mut offline,
        UiCommand::SetDndUntil(500)
    ));

    assert_eq!(offline.len(), 1);
    assert!(matches!(offline[0], UiCommand::SetDndUntil(500)));
}

#[test]
fn enqueue_offline_command_rejects_live_reply_text_and_reports_failure() {
    let mut offline = VecDeque::new();
    let (outcome, mut result) = tokio::sync::oneshot::channel();

    assert!(!enqueue_offline_command(
        &mut offline,
        UiCommand::Reply {
            id: 7,
            generation: 11,
            text: "Still there?".to_string(),
            outcome,
        }
    ));

    assert!(offline.is_empty());
    assert!(matches!(
        result.try_recv(),
        Ok(Err(message)) if message.contains("unavailable")
    ));
}
