#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    type TestResult = Result<(), Box<dyn Error>>;

    fn installer_command_as_non_root() -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_unixnotis-installer"));

        // Root-based CI must exercise the same user-level entrypoint as a desktop session
        if rustix::process::geteuid().is_root() {
            command.uid(65_534);
        }

        command
    }

    #[test]
    fn installer_help_prints_usage_from_entrypoint() -> TestResult {
        let output = installer_command_as_non_root().arg("--help").output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Usage: unixnotis-installer"));
        assert!(stdout.contains("--service-manager"));
        Ok(())
    }
}
