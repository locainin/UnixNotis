use super::handle_local_command;
use crate::cli::Command;

#[test]
fn daemon_command_fails_closed_in_local_dispatcher() {
    let error = handle_local_command(Command::OpenPanel)
        .expect_err("daemon command must fail in local dispatcher");
    assert!(
        error.to_string().contains("internal routing error"),
        "routing failure should remain visible"
    );
}
