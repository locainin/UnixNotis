use tracing::warn;

use super::super::super::Config;
use crate::{program_in_path, util};

pub(super) fn warn_missing_shell(config: &Config) {
    // Only warn when the config actually depends on shell syntax
    if program_in_path("sh") {
        return;
    }
    if !config_requires_shell(config) {
        return;
    }

    // Shell-backed commands depend on sh for pipes, redirects, and control flow
    warn!("POSIX shell (sh) not found in PATH; widget commands using pipes or redirects will fail");
}

fn config_requires_shell(config: &Config) -> bool {
    // Walk every configured command once so the warning stays accurate
    let volume = &config.widgets.volume;
    if command_requires_shell(&volume.get_cmd)
        || command_requires_shell(&volume.set_cmd)
        || command_requires_shell_opt(&volume.toggle_cmd)
        || command_requires_shell_opt(&volume.watch_cmd)
    {
        return true;
    }

    let brightness = &config.widgets.brightness;
    if command_requires_shell(&brightness.get_cmd)
        || command_requires_shell(&brightness.set_cmd)
        || command_requires_shell_opt(&brightness.toggle_cmd)
        || command_requires_shell_opt(&brightness.watch_cmd)
    {
        return true;
    }

    if config.widgets.toggles.iter().any(|toggle| {
        command_requires_shell_opt(&toggle.state_cmd)
            || command_requires_shell_opt(&toggle.toggle_cmd)
            || command_requires_shell_opt(&toggle.on_cmd)
            || command_requires_shell_opt(&toggle.off_cmd)
            || command_requires_shell_opt(&toggle.watch_cmd)
    }) {
        return true;
    }

    if config.widgets.stats.iter().any(|stat| {
        command_requires_shell_opt(&stat.cmd)
            || stat
                .plugin
                .as_ref()
                .is_some_and(|plugin| command_requires_shell(&plugin.command))
    }) {
        return true;
    }

    config.widgets.cards.iter().any(|card| {
        command_requires_shell_opt(&card.cmd)
            || card
                .plugin
                .as_ref()
                .is_some_and(|plugin| command_requires_shell(&plugin.command))
    })
}

fn command_requires_shell_opt(value: &Option<String>) -> bool {
    value
        .as_deref()
        .map(command_requires_shell)
        .unwrap_or(false)
}

fn command_requires_shell(cmd: &str) -> bool {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return false;
    }

    // Strip known runtime placeholders so braces do not trigger false positives
    let cmd = cmd.replace("{value}", "0");
    !util::is_simple_command(&cmd)
}
