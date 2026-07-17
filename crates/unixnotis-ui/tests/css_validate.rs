#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fmt::Write as _;
    use std::io::{Error as IoError, ErrorKind, Write as _};
    use std::process::{Command, Output, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};

    type TestResult = Result<(), Box<dyn Error>>;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn css_validate_accepts_parseable_css() -> TestResult {
        let output = run_validator(".panel { color: #ffffff; }")?;

        // Valid CSS should not emit parser diagnostics or fail the helper process
        assert!(output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).trim().is_empty(),
            "unexpected stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        Ok(())
    }

    #[test]
    fn css_validate_rejects_invalid_css_with_diagnostic() -> TestResult {
        let output = run_validator(".panel { color: ;")?;
        let stderr = String::from_utf8_lossy(&output.stderr);

        // A real parser error must fail so generated CSS tests catch broken output
        assert!(!output.status.success());
        assert!(stderr.contains("gtk css parse error"), "{stderr}");
        assert!(stderr.contains("gtk css validation found"), "{stderr}");

        Ok(())
    }

    #[test]
    fn path_protocol_returns_structured_parser_diagnostics() -> TestResult {
        let root = temp_root("path-protocol");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        let stylesheet = root.join("broken.css");
        std::fs::write(&stylesheet, ".panel { color: ;")?;

        let output = Command::new(env!("CARGO_BIN_EXE_unixnotis-css-validate"))
            .arg("--json-path")
            .arg(&stylesheet)
            .stdin(Stdio::null())
            .output()?;
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;

        assert!(output.status.success());
        if report["available"] == true {
            let diagnostics = report["diagnostics"]
                .as_array()
                .ok_or("diagnostics must be an array")?;
            assert!(!diagnostics.is_empty());
            assert_eq!(diagnostics[0]["line"], 1);
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn path_protocol_caps_large_diagnostic_sets() -> TestResult {
        let root = temp_root("diagnostic-cap");
        std::fs::create_dir_all(&root)?;
        let stylesheet = root.join("many-errors.css");
        let mut css = String::new();
        for index in 0..12 {
            writeln!(css, ".broken-{index} {{ color: ; }}")?;
        }
        std::fs::write(&stylesheet, css)?;

        let output = Command::new(env!("CARGO_BIN_EXE_unixnotis-css-validate"))
            .arg("--json-path")
            .arg(&stylesheet)
            .stdin(Stdio::null())
            .output()?;
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;

        assert!(output.status.success());
        if report["available"] == true {
            let diagnostics = report["diagnostics"]
                .as_array()
                .ok_or("diagnostics must be an array")?;
            assert!(diagnostics.len() <= 4);
            assert_eq!(report["truncated"], true);
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    fn run_validator(css: &str) -> Result<Output, IoError> {
        let binary = env!("CARGO_BIN_EXE_unixnotis-css-validate");
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

        child.wait_with_output()
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let serial = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "unixnotis-css-validator-{name}-{}-{serial}",
            std::process::id()
        ))
    }
}
