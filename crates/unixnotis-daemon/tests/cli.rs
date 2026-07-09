#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::process::Command;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn daemon_help_prints_usage_from_entrypoint() -> TestResult {
        let output = Command::new(env!("CARGO_BIN_EXE_unixnotis-daemon"))
            .arg("--help")
            .output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Usage:"));
        assert!(stdout.contains("--check"));
        assert!(stdout.contains("--trial"));
        Ok(())
    }
}
