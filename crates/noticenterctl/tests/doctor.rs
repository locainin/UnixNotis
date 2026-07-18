#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn binary_doctor_runs_all_checks_and_emits_versioned_json() -> TestResult {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = std::env::temp_dir().join(format!(
            "unixnotis-doctor-binary-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root)?;
        let config = root.join("config.toml");
        std::fs::write(&config, "config_version = 2\n")?;
        let missing_bus = root.join("missing-session-bus.sock");

        // A missing private bus keeps the integration deterministic without touching the desktop
        let output = Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
            .args(["doctor", "--json", "--service-manager", "manual"])
            .env("UNIXNOTIS_CONFIG_PATH", &config)
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}", missing_bus.display()),
            )
            .output()?;

        assert!(!output.status.success());
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["schema_version"], 1);
        assert!(report["checks"].as_array().is_some_and(|checks| checks
            .iter()
            .any(|check| { check["id"] == "dbus.session" && check["severity"] == "error" })));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
