use std::fs;
use std::io::Cursor;
use std::time::Duration;

use super::super::model::{CachedDiagnosticSource, CssFileIdentity, CssParseWorkItem};
use super::super::parse::{
    classify_cached_source_path, css_validator_binary_from, decode_validator_report,
    is_executable_regular_file, parse_css_file_with_gtk, read_bounded_pipe,
    replace_validator_override, run_css_validator, source_line_text, MAX_VALIDATOR_OUTPUT_BYTES,
};
use super::helpers::TempDirGuard;

#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt as _};

struct ValidatorOverrideGuard(Option<std::path::PathBuf>);

impl ValidatorOverrideGuard {
    fn install(path: std::path::PathBuf) -> Self {
        Self(replace_validator_override(Some(path)))
    }
}

impl Drop for ValidatorOverrideGuard {
    fn drop(&mut self) {
        let previous = self.0.take();
        let _ = replace_validator_override(previous);
    }
}

fn work_item_for(path: &std::path::Path) -> CssParseWorkItem {
    let metadata = fs::metadata(path).expect("read stylesheet metadata");
    CssParseWorkItem {
        load_path: path.to_path_buf(),
        canonical_path: fs::canonicalize(path).expect("resolve stylesheet"),
        identity: CssFileIdentity::from_metadata(&metadata).expect("build file identity"),
        content_hash: String::new(),
        dependencies: Vec::new(),
    }
}

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

#[cfg(unix)]
#[test]
fn production_css_parser_honors_helper_status_and_payload() {
    let root = TempDirGuard::new("validator-production-wrapper");
    let stylesheet = root.write("style.css", ".panel { color: red; }");
    let validator = root.path().join("validator");
    let report = r#"{"available":true,"error":null,"truncated":false,"diagnostics":[{"source":null,"line":1,"column":2,"message":"bad rule"}]}"#;
    fs::write(
        &validator,
        format!("#!/bin/sh\nprintf '%s\\n' '{report}'\n"),
    )
    .expect("write successful validator");
    fs::set_permissions(&validator, fs::Permissions::from_mode(0o700))
        .expect("mark validator executable");
    let _override_guard = ValidatorOverrideGuard::install(validator.clone());
    let work_item = work_item_for(&stylesheet);

    let diagnostics =
        parse_css_file_with_gtk(&work_item).expect("successful helper report should decode");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, "bad rule");

    fs::write(
        &validator,
        format!("#!/bin/sh\nprintf '%s\\n' '{report}'\nexit 9\n"),
    )
    .expect("write failing validator");
    let error = parse_css_file_with_gtk(&work_item)
        .expect_err("nonzero helper status must reject an otherwise valid payload");
    assert!(error.to_string().contains("exited with"));
}

#[test]
fn validator_pipe_reader_rejects_output_over_its_limit() {
    let error = read_bounded_pipe(Some(Cursor::new(vec![b'x'; 9])), 8, "test output")
        .expect_err("oversized validator output must fail");

    assert!(error.to_string().contains("exceeded 8 bytes"));
}

#[test]
fn validator_pipe_reader_accepts_output_at_its_exact_limit() {
    let bytes = read_bounded_pipe(Some(Cursor::new(vec![b'x'; 8])), 8, "test output")
        .expect("exact validator output limit should remain valid");

    assert_eq!(bytes, vec![b'x'; 8]);
}

#[test]
fn validator_protocol_keeps_the_documented_output_budget() {
    assert_eq!(MAX_VALIDATOR_OUTPUT_BYTES, 65_536);
}

#[test]
fn source_line_lookup_uses_one_based_parser_locations() {
    let root = TempDirGuard::new("validator-source-lines");
    let path = root.write("style.css", "first\nsecond\nthird\n");

    assert_eq!(source_line_text(Some(&path), 1).as_deref(), Some("first"));
    assert_eq!(source_line_text(Some(&path), 2).as_deref(), Some("second"));
    assert_eq!(source_line_text(Some(&path), 3).as_deref(), Some("third"));
    assert_eq!(source_line_text(Some(&path), 0), None);
    assert_eq!(source_line_text(Some(&path), 4), None);
    assert_eq!(source_line_text(None, 1), None);
}

#[test]
fn cached_source_classification_distinguishes_data_top_level_and_imports() {
    let root = TempDirGuard::new("validator-source-classification");
    let current = root.write("base.css", ".base {}");
    let imported = root.write("imported.css", ".imported {}");
    let canonical_current = fs::canonicalize(&current).expect("resolve current stylesheet");

    assert_eq!(
        classify_cached_source_path(None, &canonical_current),
        CachedDiagnosticSource::Data
    );
    assert_eq!(
        classify_cached_source_path(Some(&current), &canonical_current),
        CachedDiagnosticSource::TopLevel
    );
    assert_eq!(
        classify_cached_source_path(Some(&imported), &canonical_current),
        CachedDiagnosticSource::Path(imported)
    );
}

#[test]
fn truncated_validator_report_adds_an_explicit_finding() {
    let root = TempDirGuard::new("validator-truncation");
    let path = root.write("style.css", ".panel {}");
    let work_item = work_item_for(&path);

    let diagnostics = decode_validator_report(
        br#"{"available":true,"error":null,"truncated":true,"diagnostics":[]}"#,
        &work_item,
    )
    .expect("decode validator report");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("omitted"));
    assert_eq!(diagnostics[0].line, None);
}
