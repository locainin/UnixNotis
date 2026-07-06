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
    fn installer_help_prints_usage_from_entrypoint() -> TestResult {
        let output = Command::new(env!("CARGO_BIN_EXE_unixnotis-installer"))
            .arg("--help")
            .output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Usage: unixnotis-installer"));
        assert!(stdout.contains("--service-manager"));
        Ok(())
    }
}
