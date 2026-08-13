use clap::Parser;

use super::super::{
    Args, Command, DevCommand, DoctorCommand, DoctorServiceManagerArg, ExecutionKind,
    PresetCommand, ThemeCommand,
};

#[test]
fn execution_kind_distinguishes_sync_async_and_daemon_commands() {
    assert_eq!(
        Command::CssCheck { config: None }.execution_kind(),
        ExecutionKind::LocalSync
    );
    assert_eq!(
        Command::Doctor {
            command: None,
            json: false,
            verbose: false,
            service_manager: DoctorServiceManagerArg::Auto,
            config: None,
        }
        .execution_kind(),
        ExecutionKind::LocalAsync
    );
    assert_eq!(
        Command::Doctor {
            command: Some(DoctorCommand::RepairSession),
            json: false,
            verbose: false,
            service_manager: DoctorServiceManagerArg::Auto,
            config: None,
        }
        .execution_kind(),
        ExecutionKind::LocalSync
    );
    assert_eq!(
        Command::Dev {
            command: DevCommand::Logs,
        }
        .execution_kind(),
        ExecutionKind::LocalSync
    );
    assert_eq!(
        Command::Dev {
            command: DevCommand::DumpActive,
        }
        .execution_kind(),
        ExecutionKind::Daemon
    );
    assert_eq!(Command::ClearActive.execution_kind(), ExecutionKind::Daemon);
}

#[test]
fn preset_and_theme_commands_are_local_sync() {
    assert_eq!(
        Command::Preset {
            command: PresetCommand::Inspect {
                input: "bundle.unixnotis".to_string()
            }
        }
        .execution_kind(),
        ExecutionKind::LocalSync
    );
    assert_eq!(
        Command::Theme {
            command: ThemeCommand::ExportStock { output: None }
        }
        .execution_kind(),
        ExecutionKind::LocalSync
    );
}

#[test]
fn parsed_preset_command_bypasses_daemon_bootstrap() {
    let args = Args::try_parse_from(["noticenterctl", "preset", "inspect", "bundle.unixnotis"])
        .expect("parse args");
    assert_eq!(args.command.execution_kind(), ExecutionKind::LocalSync);
}

#[test]
fn doctor_report_is_local_async() {
    assert_eq!(
        Command::Doctor {
            command: None,
            json: false,
            verbose: false,
            service_manager: DoctorServiceManagerArg::Auto,
            config: None,
        }
        .execution_kind(),
        ExecutionKind::LocalAsync
    );
}

#[test]
fn theme_export_stock_is_local_and_accepts_an_optional_directory() {
    let args = Args::try_parse_from([
        "noticenterctl",
        "theme",
        "export-stock",
        "--output",
        "editable-theme",
    ])
    .expect("theme export arguments should parse");

    let Command::Theme {
        command: ThemeCommand::ExportStock { output },
    } = args.command
    else {
        panic!("theme export command should be selected");
    };
    assert_eq!(output, Some("editable-theme".into()));
}
