use std::fs;
use std::io::Cursor;
use std::time::Duration;

use super::super::model::{CssFileIdentity, CssParseWorkItem};
use super::super::parse::{
    css_validator_binary_from, decode_validator_report, is_executable_regular_file,
    read_bounded_pipe, run_css_validator,
};
use super::helpers::TempDirGuard;

#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt as _};

#[cfg(unix)]
#[test]
fn validator_lookup_accepts_only_executable_regular_files() {
    let root = TempDirGuard::new("validator-lookup");
    let executable = root.path().join("unixnotis-css-validate");
    fs::write(&executable, "#!/bin/sh\n").expect("write validator fixture");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
        .expect("mark validator executable");

    let noticenterctl = root.path().join("noticenterctl");
    assert_eq!(
        css_validator_binary_from(&noticenterctl).expect("find validator"),
        executable
    );

    fs::set_permissions(&executable, fs::Permissions::from_mode(0o600))
        .expect("remove executable permission");
    assert!(css_validator_binary_from(&noticenterctl).is_err());
}

#[cfg(unix)]
#[test]
fn validator_lookup_rejects_symlink_candidates() {
    let root = TempDirGuard::new("validator-symlink");
    let target = root.path().join("real-validator");
    fs::write(&target, "#!/bin/sh\n").expect("write validator target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700))
        .expect("mark validator target executable");

    let candidate = root.path().join("unixnotis-css-validate");
    symlink(&target, &candidate).expect("link validator candidate");

    assert!(!is_executable_regular_file(&candidate));
    assert!(css_validator_binary_from(&root.path().join("noticenterctl")).is_err());
}

#[cfg(unix)]
#[test]
fn validator_lookup_checks_test_binary_parent() {
    let root = TempDirGuard::new("validator-test-parent");
    let deps = root.path().join("deps");
    fs::create_dir(&deps).expect("create deps directory");
    let executable = root.path().join("unixnotis-css-validate");
    fs::write(&executable, "#!/bin/sh\n").expect("write validator fixture");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
        .expect("mark validator executable");

    assert_eq!(
        css_validator_binary_from(&deps.join("noticenterctl-test")).expect("find validator"),
        executable
    );
}

#[cfg(unix)]
#[test]
fn validator_process_is_stopped_after_its_deadline() {
    let root = TempDirGuard::new("validator-timeout");
    let executable = root.path().join("unixnotis-css-validate");
    fs::write(&executable, "#!/bin/sh\nwhile :; do :; done\n").expect("write validator fixture");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
        .expect("mark validator executable");

    let error = run_css_validator(
        &executable,
        &root.path().join("style.css"),
        Duration::from_millis(20),
    )
    .expect_err("non-terminating validator must time out");

    assert!(error.to_string().contains("deadline"));
}

#[test]
fn validator_pipe_reader_rejects_output_over_its_limit() {
    let error = read_bounded_pipe(Some(Cursor::new(vec![b'x'; 9])), 8, "test output")
        .expect_err("oversized validator output must fail");

    assert!(error.to_string().contains("exceeded 8 bytes"));
}

#[test]
fn truncated_validator_report_adds_an_explicit_finding() {
    let root = TempDirGuard::new("validator-truncation");
    let path = root.write("style.css", ".panel {}");
    let metadata = fs::metadata(&path).expect("read stylesheet metadata");
    let work_item = CssParseWorkItem {
        load_path: path.clone(),
        canonical_path: fs::canonicalize(&path).expect("resolve stylesheet"),
        identity: CssFileIdentity::from_metadata(&metadata).expect("build file identity"),
        content_hash: String::new(),
        dependencies: Vec::new(),
    };

    let diagnostics = decode_validator_report(
        br#"{"available":true,"error":null,"truncated":true,"diagnostics":[]}"#,
        &work_item,
    )
    .expect("decode validator report");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("omitted"));
    assert_eq!(diagnostics[0].line, None);
}
