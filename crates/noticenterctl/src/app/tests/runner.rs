use std::cell::Cell;

use anyhow::Result;

use crate::cli::{Command, PresetCommand};

use super::local::handle_local_command;

#[test]
fn handle_local_command_runs_css_check_branch() {
    let css_called = Cell::new(false);

    handle_local_command(
        Command::CssCheck { config: None },
        |config| {
            assert!(config.is_none());
            css_called.set(true);
            Ok(())
        },
        |_| -> Result<()> { panic!("preset runner should not be called for css check") },
        |_| -> Result<()> { panic!("session runner should not be called for css check") },
        |_| -> Result<()> { panic!("theme runner should not be called for css check") },
    )
    .expect("css check should dispatch");

    assert!(css_called.get());
}

#[test]
fn handle_local_command_runs_preset_branch_with_command_payload() {
    let preset_called = Cell::new(false);

    handle_local_command(
        Command::Preset {
            command: PresetCommand::Inspect {
                input: "theme.unixnotis".to_string(),
            },
        },
        |_| -> Result<()> { panic!("css runner should not be called for preset command") },
        |command| {
            let PresetCommand::Inspect { input } = command else {
                panic!("expected inspect preset command");
            };
            assert_eq!(input, "theme.unixnotis");
            preset_called.set(true);
            Ok(())
        },
        |_| -> Result<()> { panic!("session runner should not be called for preset command") },
        |_| -> Result<()> { panic!("theme runner should not be called for preset command") },
    )
    .expect("preset should dispatch");

    assert!(preset_called.get());
}
