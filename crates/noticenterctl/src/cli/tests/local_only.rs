use clap::Parser;

use super::super::{Args, Command, PresetCommand};

#[test]
fn local_only_classification_distinguishes_local_and_control_commands() {
    assert!(Command::CssCheck.is_local_only());
    assert!(Command::Preset {
        command: PresetCommand::Inspect {
            input: "bundle.unixnotis".to_string()
        }
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
