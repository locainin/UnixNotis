use clap::Parser;
use unixnotis_core::PanelDebugLevel;

use crate::cli::{
    Args, Command, DevCommand, DndState, DoctorServiceManagerArg, InhibitScopeArg, PresetCommand,
};

use super::super::commands::{handle_command, handle_dev_command_with_diagnostic_mode};
use super::support::{RecordedCall, RecordingControlClient};

#[tokio::test]
async fn clear_commands_dispatch_to_matching_control_calls() {
    let cases = [
        (Command::Clear, RecordedCall::ClearAll),
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
        (Command::OpenPanel, RecordedCall::OpenPanel),
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
async fn dev_commands_dispatch_to_exact_control_calls() {
    // Build the debug command through Clap so the test covers the public CLI path
    let parsed = Args::try_parse_from(["noticenterctl", "dev", "open-panel", "--level", "verbose"])
        .expect("parse developer panel command");
    let Command::Dev {
        command: open_panel,
    } = parsed.command
    else {
        panic!("developer panel command should be selected");
    };

    let cases = [
        (
            open_panel,
            RecordedCall::OpenPanelDebug(PanelDebugLevel::Verbose),
        ),
        (
            DevCommand::RefreshApplications,
            RecordedCall::RefreshApplications,
        ),
        (
            DevCommand::ExplainNotification { id: 8 },
            RecordedCall::NotificationDiagnostics(8),
        ),
    ];

    for (command, expected) in cases {
        let client = RecordingControlClient::default();
        handle_command(&client, Command::Dev { command })
            .await
            .expect("dispatch developer command");
        assert_eq!(client.take_calls(), vec![expected]);
    }
}

#[tokio::test]
async fn dnd_commands_dispatch_to_matching_control_calls() {
    let cases = [
        (
            Command::Dnd {
                state: DndState::On,
                for_duration: None,
                until: None,
            },
            RecordedCall::SetDnd(true),
        ),
        (
            Command::Dnd {
                state: DndState::Off,
                for_duration: None,
                until: None,
            },
            RecordedCall::SetDnd(false),
        ),
        (
            Command::Dnd {
                state: DndState::Toggle,
                for_duration: None,
                until: None,
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
async fn timed_dnd_dispatches_one_future_absolute_deadline() {
    use std::str::FromStr;

    let client = RecordingControlClient::default();
    let before = chrono::Utc::now().timestamp();
    handle_command(
        &client,
        Command::Dnd {
            state: DndState::On,
            for_duration: Some(crate::cli::DndDuration::from_str("30m").expect("valid duration")),
            until: None,
        },
    )
    .await
    .expect("dispatch timed DND");
    let after = chrono::Utc::now().timestamp();

    let calls = client.take_calls();
    let [RecordedCall::SetDndUntil(expires_at)] = calls.as_slice() else {
        panic!("expected one timed DND call, got {calls:?}");
    };
    assert!(*expires_at >= before + 30 * 60);
    assert!(*expires_at <= after + 30 * 60);
}

#[tokio::test]
async fn timed_dnd_dispatch_rejects_non_on_state_without_calling_control() {
    use std::str::FromStr;

    let client = RecordingControlClient::default();
    let result = handle_command(
        &client,
        Command::Dnd {
            state: DndState::Off,
            for_duration: Some(crate::cli::DndDuration::from_str("30m").expect("valid duration")),
            until: None,
        },
    )
    .await;

    assert!(result.is_err());
    assert!(client.take_calls().is_empty());
}

#[tokio::test]
async fn notification_commands_dispatch_to_matching_control_calls() {
    let cases = [
        (Command::Dismiss { id: 7 }, RecordedCall::Dismiss(7)),
        (Command::ListActive, RecordedCall::ListActive),
        (Command::ListHistory, RecordedCall::ListHistory),
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
async fn diagnostic_dumps_require_diagnostic_mode_before_fetching_data() {
    for command in [DevCommand::DumpActive, DevCommand::DumpHistory] {
        let client = RecordingControlClient::default();
        let error = handle_dev_command_with_diagnostic_mode(&client, command, false)
            .await
            .expect_err("diagnostic dump should be rejected");

        assert!(error.to_string().contains("UNIXNOTIS_DIAGNOSTIC=1"));
        assert!(client.take_calls().is_empty());
    }
}

#[tokio::test]
async fn diagnostic_dumps_fetch_matching_data_when_mode_is_enabled() {
    let cases = [
        (DevCommand::DumpActive, RecordedCall::ListActive),
        (DevCommand::DumpHistory, RecordedCall::ListHistory),
    ];

    for (command, expected) in cases {
        let client = RecordingControlClient::default();
        handle_dev_command_with_diagnostic_mode(&client, command, true)
            .await
            .expect("diagnostic dump should dispatch");
        assert_eq!(client.take_calls(), vec![expected]);
    }
}

#[tokio::test]
async fn local_commands_fail_closed_without_touching_control_client() {
    let cases = [
        Command::CssCheck { config: None },
        Command::Doctor {
            command: None,
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
        let error = handle_command(&client, command)
            .await
            .expect_err("local command must fail in daemon dispatcher");
        assert!(error.to_string().contains("internal routing error"));
        assert!(client.take_calls().is_empty());
    }
}

#[tokio::test]
async fn dev_logs_fail_closed_without_touching_control_client() {
    let client = RecordingControlClient::default();
    let error = handle_command(
        &client,
        Command::Dev {
            command: DevCommand::Logs,
        },
    )
    .await
    .expect_err("local log follower must fail in daemon dispatcher");

    assert!(error.to_string().contains("internal routing error"));
    assert!(client.take_calls().is_empty());
}
