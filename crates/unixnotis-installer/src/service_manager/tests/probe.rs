use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::service_manager::{use_fake_command_bin, CommandSpec, ServiceProbe};

#[test]
fn stdout_probe_uses_parser_only_after_successful_command() {
    let root = test_root("stdout-probe-success");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    let probe_bin = fake_bin.join("probe");
    fs::write(&probe_bin, "#!/bin/sh\nprintf 'true\\n'\nexit 0\n").expect("fake probe");
    fs::set_permissions(&probe_bin, fs::Permissions::from_mode(0o755)).expect("fake probe mode");
    let _fake = use_fake_command_bin(&fake_bin);
    let probe = ServiceProbe::stdout(
        CommandSpec::new("probe", "probe", std::iter::empty::<&str>()),
        |stdout| stdout.trim() == "true",
    );

    let active = probe.evaluate().expect("probe should run");

    // Successful stdout probes are allowed to derive active state from command output
    assert!(active);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn stdout_probe_treats_failed_command_as_inactive_even_with_matching_output() {
    let root = test_root("stdout-probe-failure");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    let probe_bin = fake_bin.join("probe");
    fs::write(&probe_bin, "#!/bin/sh\nprintf 'true\\n'\nexit 1\n").expect("fake probe");
    fs::set_permissions(&probe_bin, fs::Permissions::from_mode(0o755)).expect("fake probe mode");
    let _fake = use_fake_command_bin(&fake_bin);
    let probe = ServiceProbe::stdout(
        CommandSpec::new("probe", "probe", std::iter::empty::<&str>()),
        |stdout| stdout.trim() == "true",
    );

    let active = probe.evaluate().expect("probe should run");

    // Command failure means the manager did not provide trustworthy status output
    assert!(!active);
    let _ = fs::remove_dir_all(root);
}

fn test_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
