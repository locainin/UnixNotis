#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::process::Command;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn binary_rejects_unknown_command_before_dbus_setup() -> TestResult {
        let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
            .arg("definitely-not-a-command")
            .output()?;

        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains("unrecognized subcommand"));
        assert!(stderr.contains("definitely-not-a-command"));
        Ok(())
    }

    #[test]
    fn binary_rejects_invalid_dnd_state_before_dbus_setup() -> TestResult {
        let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
            .args(["dnd", "maybe"])
            .output()?;

        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains("invalid value"));
        assert!(stderr.contains("maybe"));
        Ok(())
    }
}
