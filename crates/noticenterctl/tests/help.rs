#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::restriction,
    reason = "workspace CI enables clippy::restriction as a review signal"
)]

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::process::Command;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn binary_help_prints_cli_usage() -> TestResult {
        let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
            .arg("--help")
            .output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Usage:"));
        assert!(stdout.contains("css-check"));
        assert!(stdout.contains("preset"));
        Ok(())
    }

    #[test]
    fn binary_open_panel_help_lists_optional_debug_flag() -> TestResult {
        let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
            .args(["open-panel", "--help"])
            .output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("--debug"));
        assert!(stdout.contains("critical"));
        assert!(stdout.contains("verbose"));
        Ok(())
    }

    #[test]
    fn binary_preset_help_lists_local_bundle_commands() -> TestResult {
        let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
            .args(["preset", "--help"])
            .output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("export"));
        assert!(stdout.contains("import"));
        assert!(stdout.contains("inspect"));
        Ok(())
    }
}
