use std::io::Write;
use std::process::{Command, Stdio};

fn validator_command() -> Command {
    let binary = env!("CARGO_BIN_EXE_css_provider_validate");

    // The validator is tested as a real child process because its contract is
    // stdin, stderr diagnostics, and an exit status used by CSS tests
    Command::new(binary)
}

#[test]
fn css_provider_validate_accepts_parseable_css() {
    let output = run_validator(".panel { color: #ffffff; }");

    // Valid CSS should not emit parser diagnostics or fail the helper process
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).trim().is_empty(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn css_provider_validate_rejects_invalid_css_with_diagnostic() {
    let output = run_validator(".panel { color: ;");
    let stderr = String::from_utf8_lossy(&output.stderr);

    // A real parser error must fail so generated CSS tests catch broken output
    assert!(!output.status.success());
    assert!(stderr.contains("gtk css parse error"), "{stderr}");
    assert!(stderr.contains("gtk css validation found"), "{stderr}");
}

fn run_validator(css: &str) -> std::process::Output {
    let mut child = validator_command()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn css validator");

    child
        .stdin
        .take()
        .expect("css validator stdin")
        .write_all(css.as_bytes())
        .expect("write css input");

    child.wait_with_output().expect("wait for css validator")
}
