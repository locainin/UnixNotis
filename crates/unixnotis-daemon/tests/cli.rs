#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::os::unix::net::UnixListener;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

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

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> TestResult<Self> {
            let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = std::env::temp_dir().join(format!(
                "unixnotis-daemon-cli-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path)?;
            Ok(Self(path))
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn daemon_runtime_reports_an_unreachable_session_bus() -> TestResult {
        let root = TempRoot::new()?;
        let config = root.path().join("config.toml");
        fs::write(&config, "config_version = 3\n")?;
        let display = "wayland-unixnotis-test";
        let _wayland = UnixListener::bind(root.path().join(display))?;

        let output = Command::new(env!("CARGO_BIN_EXE_unixnotis-daemon"))
            .args(["--config", config.to_str().ok_or("non-UTF-8 config path")?])
            .args(["--run-seconds", "0"])
            .env("XDG_RUNTIME_DIR", root.path())
            .env("WAYLAND_DISPLAY", display)
            .env("XDG_SESSION_TYPE", "wayland")
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                "unix:path=/nonexistent/unixnotis-test-session-bus",
            )
            .output()?;

        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains("session bus"), "unexpected error: {stderr}");
        Ok(())
    }
}
