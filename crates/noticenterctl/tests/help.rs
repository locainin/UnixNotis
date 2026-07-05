use std::process::Command;

#[test]
fn binary_help_prints_cli_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
        .arg("--help")
        .output()
        .expect("run noticenterctl --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help output is utf8");
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("css-check"));
    assert!(stdout.contains("preset"));
}

#[test]
fn binary_open_panel_help_lists_optional_debug_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
        .args(["open-panel", "--help"])
        .output()
        .expect("run noticenterctl open-panel --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help output is utf8");
    assert!(stdout.contains("--debug"));
    assert!(stdout.contains("critical"));
    assert!(stdout.contains("verbose"));
}

#[test]
fn binary_preset_help_lists_local_bundle_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
        .args(["preset", "--help"])
        .output()
        .expect("run noticenterctl preset --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help output is utf8");
    assert!(stdout.contains("export"));
    assert!(stdout.contains("import"));
    assert!(stdout.contains("inspect"));
}
