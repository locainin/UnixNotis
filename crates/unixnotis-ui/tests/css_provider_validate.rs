#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::implicit_return,
    clippy::nursery,
    clippy::needless_return,
    clippy::panic_in_result_fn,
    clippy::pedantic,
    clippy::question_mark_used,
    clippy::restriction,
    clippy::std_instead_of_core,
    reason = "workspace clippy runs use these groups as review signals, not as zero-tolerance policy gates"
)]

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{Error as IoError, ErrorKind, Write as _};
    use std::process::{Command, Output, Stdio};

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn css_provider_validate_accepts_parseable_css() -> TestResult {
        let output = run_validator(".panel { color: #ffffff; }")?;

        // Valid CSS should not emit parser diagnostics or fail the helper process
        assert!(output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).trim().is_empty(),
            "unexpected stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        return Ok(());
    }

    #[test]
    fn css_provider_validate_rejects_invalid_css_with_diagnostic() -> TestResult {
        let output = run_validator(".panel { color: ;")?;
        let stderr = String::from_utf8_lossy(&output.stderr);

        // A real parser error must fail so generated CSS tests catch broken output
        assert!(!output.status.success());
        assert!(stderr.contains("gtk css parse error"), "{stderr}");
        assert!(stderr.contains("gtk css validation found"), "{stderr}");

        return Ok(());
    }

    fn run_validator(css: &str) -> Result<Output, IoError> {
        let binary = env!("CARGO_BIN_EXE_css_provider_validate");
        let mut child = Command::new(binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        // The validator contract is stdin, stderr diagnostics, and exit status
        let Some(mut stdin) = child.stdin.take() else {
            return Err(IoError::new(
                ErrorKind::BrokenPipe,
                "css validator stdin unavailable",
            ));
        };
        stdin.write_all(css.as_bytes())?;
        drop(stdin);

        return child.wait_with_output();
    }
}
