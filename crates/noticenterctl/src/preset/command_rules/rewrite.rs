use std::path::Path;

use unixnotis_core::{Config, SliderWidgetConfig};

use super::checks::collect_host_specific_command_paths;
use super::tokens::rewrite_command_to_config_relative;
use super::HostSpecificCommandPath;

pub(crate) fn rewrite_host_specific_command_paths(
    config_dir: &Path,
    config: &mut Config,
) -> Vec<HostSpecificCommandPath> {
    let leaks = collect_host_specific_command_paths(config_dir, config);
    if leaks.is_empty() {
        return leaks;
    }

    // Each command-bearing slot is rewritten in place so only the exported bundle changes
    rewrite_slider_commands(config_dir, &mut config.widgets.volume);
    rewrite_slider_commands(config_dir, &mut config.widgets.brightness);
    for toggle in &mut config.widgets.toggles {
        rewrite_optional_command(config_dir, &mut toggle.state_cmd);
        rewrite_optional_command(config_dir, &mut toggle.on_cmd);
        rewrite_optional_command(config_dir, &mut toggle.off_cmd);
        rewrite_optional_command(config_dir, &mut toggle.watch_cmd);
    }
    for stat in &mut config.widgets.stats {
        rewrite_optional_command(config_dir, &mut stat.cmd);
        if let Some(plugin) = stat.plugin.as_mut() {
            rewrite_inline_command(config_dir, &mut plugin.command);
        }
    }
    for card in &mut config.widgets.cards {
        rewrite_optional_command(config_dir, &mut card.cmd);
        if let Some(plugin) = card.plugin.as_mut() {
            rewrite_inline_command(config_dir, &mut plugin.command);
        }
    }

    leaks
}

fn rewrite_slider_commands(config_dir: &Path, slider: &mut SliderWidgetConfig) {
    rewrite_inline_command(config_dir, &mut slider.get_cmd);
    rewrite_inline_command(config_dir, &mut slider.set_cmd);
    rewrite_optional_command(config_dir, &mut slider.toggle_cmd);
    rewrite_optional_command(config_dir, &mut slider.watch_cmd);
}

fn rewrite_optional_command(config_dir: &Path, value: &mut Option<String>) {
    let Some(command) = value.as_mut() else {
        return;
    };
    rewrite_inline_command(config_dir, command);
}

fn rewrite_inline_command(config_dir: &Path, command: &mut String) {
    let Some(rewritten) = rewrite_command_to_config_relative(config_dir, command) else {
        return;
    };
    *command = rewritten;
}
