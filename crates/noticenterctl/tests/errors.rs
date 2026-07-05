use std::process::Command;

#[test]
fn binary_rejects_unknown_command_before_dbus_setup() {
    let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
        .arg("definitely-not-a-command")
        .output()
        .expect("run noticenterctl invalid command");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("error output is utf8");
    assert!(stderr.contains("unrecognized subcommand"));
    assert!(stderr.contains("definitely-not-a-command"));
}

#[test]
fn binary_rejects_invalid_dnd_state_before_dbus_setup() {
    let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
        .args(["dnd", "maybe"])
        .output()
        .expect("run noticenterctl invalid dnd state");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("error output is utf8");
    assert!(stderr.contains("invalid value"));
    assert!(stderr.contains("maybe"));
}
