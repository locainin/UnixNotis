#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    type TestResult = Result<(), Box<dyn Error>>;

    static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(name: &str) -> Result<Self, Box<dyn Error>> {
            // Unique roots keep binary tests from sharing config state through XDG_CONFIG_HOME
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "unixnotis-cli-preset-inspect-{name}-{stamp}-{serial}"
            ));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }

        fn write(&self, relative_path: &str, contents: &str) -> Result<(), Box<dyn Error>> {
            // The CLI reads the default config root, so tests place files under an isolated XDG root
            let path = self.path.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, contents)?;
            Ok(())
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn binary_preset_inspect_prints_bundle_report() -> TestResult {
        let root = TempDirGuard::new("report")?;
        let config_root = root.path.join("xdg");
        let unixnotis_root = config_root.join("unixnotis");
        root.write(
            "xdg/unixnotis/config.toml",
            "[theme]\nbase_css = \"base.css\"\n",
        )?;
        root.write("xdg/unixnotis/base.css", ".panel { color: red; }\n")?;
        let bundle_path = root.path.join("demo.unixnotis");

        let export_output = noticenterctl()
            .env("XDG_CONFIG_HOME", &config_root)
            .args(["preset", "export", "--force"])
            .arg(&bundle_path)
            .output()?;
        assert!(
            export_output.status.success(),
            "export failed: {}",
            String::from_utf8_lossy(&export_output.stderr)
        );

        let inspect_output = noticenterctl()
            .args(["preset", "inspect"])
            .arg(&bundle_path)
            .output()?;

        assert!(
            inspect_output.status.success(),
            "inspect failed: {}",
            String::from_utf8_lossy(&inspect_output.stderr)
        );
        let stdout = String::from_utf8(inspect_output.stdout)?;
        assert!(stdout.contains("preset: demo"));
        assert!(stdout.contains("files: 2"));
        assert!(stdout.contains("file list:"));
        assert!(stdout.contains("config.toml"));
        assert!(unixnotis_root.exists());
        Ok(())
    }

    fn noticenterctl() -> Command {
        // Cargo provides the freshly-built binary path to integration tests
        Command::new(env!("CARGO_BIN_EXE_noticenterctl"))
    }
}
