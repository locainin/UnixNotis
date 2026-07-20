use anyhow::Result;

use super::handle_local_command;
use crate::cli::Command;

#[test]
fn daemon_command_is_not_dispatched_to_local_handlers() {
    let mut css_called = false;
    let mut preset_called = false;

    handle_local_command(
        Command::OpenPanel { debug: None },
        |_| {
            css_called = true;
            Ok(())
        },
        |_| {
            preset_called = true;
            Ok(())
        },
        |_| Ok(()),
    )
    .expect("ignore daemon command in local dispatcher");

    assert!(!css_called, "daemon command must not invoke CSS checks");
    assert!(
        !preset_called,
        "daemon command must not invoke preset handling"
    );
}

#[test]
fn local_handler_error_is_returned_to_the_caller() {
    let result = handle_local_command(
        Command::CssCheck { config: None },
        |_| anyhow::bail!("CSS check failed"),
        |_| -> Result<()> { Ok(()) },
        |_| -> Result<()> { Ok(()) },
    );

    let error = result.expect_err("local command failure should be returned");
    assert!(
        error.to_string().contains("CSS check failed"),
        "original local command error should remain visible"
    );
}
