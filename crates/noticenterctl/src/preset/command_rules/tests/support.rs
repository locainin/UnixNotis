use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use unixnotis_core::{parse_legacy_command, CommandSpec, Config, ConfigError};

use super::super::validate_command_paths_in_config_bytes as validate_command_paths;
use crate::test_support::{current_config_bytes, current_config_text};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(super) fn temp_root(name: &str) -> PathBuf {
    // Unique paths keep lexical path checks stable under parallel test runs
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "unixnotis-preset-command-rules-{name}-{stamp}-{serial}"
    ))
}

pub(super) fn parsed_command(command: &str) -> CommandSpec {
    parse_legacy_command(command).expect("valid legacy test command")
}

pub(super) fn parse_current_config(contents: &str) -> Result<Config, ConfigError> {
    Config::parse(&current_config_text(contents))
}

pub(super) fn validate_command_paths_in_config_bytes(
    config_dir: &Path,
    config_bytes: &[u8],
    mode_label: &str,
) -> Result<()> {
    validate_command_paths(config_dir, &current_config_bytes(config_bytes), mode_label)
}
