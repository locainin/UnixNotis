use anyhow::anyhow;
use unixnotis_core::PanelDebugLevel;

use crate::cli::{
    Command, DebugLevelArg, DndState, DoctorServiceManagerArg, InhibitScopeArg, PresetCommand,
};

use super::super::commands::{handle_command, handle_command_with_debug_logs};
use super::support::{RecordedCall, RecordedEvent, RecordingControlClient};

#[tokio::test]
async fn clear_commands_dispatch_to_matching_control_calls() {
    let cases = [
        (Command::Clear, RecordedCall::ClearAll),
        (Command::ClearAll, RecordedCall::ClearAll),
        (Command::ClearActive, RecordedCall::ClearActive),
        (Command::ClearHistory, RecordedCall::ClearHistory),
    ];

    for (command, expected) in cases {
        let client = RecordingControlClient::default();
        handle_command(&client, command)
            .await
            .expect("dispatch command");
        assert_eq!(client.take_calls(), vec![expected]);
    }
}

#[tokio::test]
async fn panel_commands_dispatch_to_matching_control_calls() {
    let cases = [
        (Command::TogglePanel, RecordedCall::TogglePanel),
        (Command::OpenPanel { debug: None }, RecordedCall::OpenPanel),
        (Command::ClosePanel, RecordedCall::ClosePanel),
    ];

    for (command, expected) in cases {
        let client = RecordingControlClient::default();
        handle_command(&client, command)
            .await
            .expect("dispatch command");
        assert_eq!(client.take_calls(), vec![expected]);
    }
}

#[tokio::test]
async fn debug_open_dispatches_debug_panel_and_attempts_log_follow() {
    let client = RecordingControlClient::default();

    handle_command_with_debug_logs(
        &client,
        Command::OpenPanel {
            debug: Some(DebugLevelArg::Verbose),
        },
        || {
            client.record_debug_log_follow();
            Ok(())
        },
    )
    .await
    .expect("dispatch debug open");

    assert_eq!(
        client.take_events(),
        vec![
            RecordedEvent::Control(RecordedCall::OpenPanelDebug(PanelDebugLevel::Verbose)),
            RecordedEvent::DebugLogFollow,
        ]
    );
}

#[tokio::test]
async fn debug_open_still_succeeds_when_log_follow_fails() {
    let client = RecordingControlClient::default();

    handle_command_with_debug_logs(
        &client,
        Command::OpenPanel {
            debug: Some(DebugLevelArg::Warn),
        },
        || {
            client.record_debug_log_follow();
            Err(anyhow!("journal unavailable"))
        },
    )
    .await
    .expect("debug open should survive log follow errors");

    assert_eq!(
        client.take_events(),
        vec![
            RecordedEvent::Control(RecordedCall::OpenPanelDebug(PanelDebugLevel::Warn)),
            RecordedEvent::DebugLogFollow,
        ]
    );
}

#[tokio::test]
async fn dnd_commands_dispatch_to_matching_control_calls() {
    let cases = [
        (
            Command::Dnd {
                state: DndState::On,
            },
            RecordedCall::SetDnd(true),
        ),
        (
            Command::Dnd {
                state: DndState::Off,
            },
            RecordedCall::SetDnd(false),
        ),
        (
            Command::Dnd {
                state: DndState::Toggle,
            },
            RecordedCall::ToggleDnd,
        ),
    ];

    for (command, expected) in cases {
        let client = RecordingControlClient::default();
        handle_command(&client, command)
            .await
            .expect("dispatch command");
        assert_eq!(client.take_calls(), vec![expected]);
    }
}

#[tokio::test]
async fn notification_commands_dispatch_to_matching_control_calls() {
    let cases = [
        (Command::Dismiss { id: 7 }, RecordedCall::Dismiss(7)),
        (
            Command::ListActive { full: false },
            RecordedCall::ListActive,
        ),
        (
            Command::ListHistory { full: false },
            RecordedCall::ListHistory,
        ),
    ];

    for (command, expected) in cases {
        let client = RecordingControlClient::default();
        handle_command(&client, command)
            .await
            .expect("dispatch command");
        assert_eq!(client.take_calls(), vec![expected]);
    }
}

#[tokio::test]
async fn inhibitor_commands_dispatch_to_matching_control_calls() {
    let cases = [
        (
            Command::Inhibit {
                reason: "focus".to_string(),
                scope: InhibitScopeArg::Popups,
            },
            RecordedCall::Inhibit {
                reason: "focus".to_string(),
                scope: unixnotis_core::INHIBIT_SCOPE_POPUPS,
            },
        ),
        (Command::Uninhibit { id: 9 }, RecordedCall::Uninhibit(9)),
        (Command::ListInhibitors, RecordedCall::ListInhibitors),
    ];

    for (command, expected) in cases {
        let client = RecordingControlClient::default();
        handle_command(&client, command)
            .await
            .expect("dispatch command");
        assert_eq!(client.take_calls(), vec![expected]);
    }
}

#[tokio::test]
async fn local_commands_do_not_touch_control_client() {
    let cases = [
        Command::CssCheck { config: None },
        Command::Doctor {
            json: false,
            verbose: false,
            service_manager: DoctorServiceManagerArg::Auto,
            config: None,
        },
        Command::Preset {
            command: PresetCommand::Inspect {
                input: "bundle.unixnotis".to_string(),
            },
        },
    ];

    for command in cases {
        let client = RecordingControlClient::default();
        handle_command(&client, command)
            .await
            .expect("dispatch command");
        assert!(client.take_calls().is_empty());
    }
}
