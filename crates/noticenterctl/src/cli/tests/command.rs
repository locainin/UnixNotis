use clap::Parser;

use super::super::{Args, Command, DoctorServiceManagerArg, PresetCommand, ThemeCommand};

#[test]
fn local_only_classification_distinguishes_local_and_control_commands() {
    assert!(Command::CssCheck { config: None }.is_local_only());
    assert!(Command::Doctor {
        json: false,
        verbose: false,
        service_manager: DoctorServiceManagerArg::Auto,
        config: None,
    }
    .is_local_only());
    assert!(Command::Preset {
        command: PresetCommand::Inspect {
            input: "bundle.unixnotis".to_string()
        }
    }
    .is_local_only());
    assert!(Command::Theme {
        command: ThemeCommand::ExportStock { output: None }
    }
    .is_local_only());

    assert!(!Command::ClearActive.is_local_only());
}

#[test]
fn preset_commands_are_local_only() {
    // Preset commands should bypass D-Bus setup like css-check does
    let args = Args::try_parse_from(["noticenterctl", "preset", "inspect", "bundle.unixnotis"])
        .expect("parse args");
    assert!(args.command.is_local_only());
}

#[test]
fn synchronous_classification_builds_a_runtime_only_when_needed() {
    assert!(Command::CssCheck { config: None }.is_synchronous());
    assert!(Command::Preset {
        command: PresetCommand::Inspect {
            input: "bundle.unixnotis".to_string()
        }
    }
    .is_synchronous());
    assert!(Command::Theme {
        command: ThemeCommand::ExportStock { output: None }
    }
    .is_synchronous());

    assert!(!Command::Doctor {
        json: false,
        verbose: false,
        service_manager: DoctorServiceManagerArg::Auto,
        config: None,
    }
    .is_synchronous());
    assert!(!Command::ClearActive.is_synchronous());
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
