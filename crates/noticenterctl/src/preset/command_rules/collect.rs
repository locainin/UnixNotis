use unixnotis_core::Config;

use super::CommandReference;

pub(crate) fn collect_command_references_from_config(config: &Config) -> Vec<CommandReference> {
    let mut commands = Vec::new();

    // Each widget family is collected separately so later checks can reason about real slot names
    collect_slider_commands(
        &mut commands,
        "widgets.volume",
        &config.widgets.volume.get_cmd,
        &config.widgets.volume.set_cmd,
        config.widgets.volume.toggle_cmd.as_deref(),
        config.widgets.volume.watch_cmd.as_deref(),
    );
    collect_slider_commands(
        &mut commands,
        "widgets.brightness",
        &config.widgets.brightness.get_cmd,
        &config.widgets.brightness.set_cmd,
        config.widgets.brightness.toggle_cmd.as_deref(),
        config.widgets.brightness.watch_cmd.as_deref(),
    );
    for (index, toggle) in config.widgets.toggles.iter().enumerate() {
        push_optional_command(
            &mut commands,
            &format!("widgets.toggles[{index}].state_cmd"),
            toggle.state_cmd.as_deref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.toggles[{index}].on_cmd"),
            toggle.on_cmd.as_deref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.toggles[{index}].off_cmd"),
            toggle.off_cmd.as_deref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.toggles[{index}].watch_cmd"),
            toggle.watch_cmd.as_deref(),
        );
    }
    for (index, stat) in config.widgets.stats.iter().enumerate() {
        push_optional_command(
            &mut commands,
            &format!("widgets.stats[{index}].cmd"),
            stat.cmd.as_deref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.stats[{index}].plugin.command"),
            stat.plugin.as_ref().map(|plugin| plugin.command.as_str()),
        );
    }
    for (index, card) in config.widgets.cards.iter().enumerate() {
        push_optional_command(
            &mut commands,
            &format!("widgets.cards[{index}].cmd"),
            card.cmd.as_deref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.cards[{index}].plugin.command"),
            card.plugin.as_ref().map(|plugin| plugin.command.as_str()),
        );
    }

    commands
}

fn collect_slider_commands(
    commands: &mut Vec<CommandReference>,
    base_slot: &str,
    get_cmd: &str,
    set_cmd: &str,
    toggle_cmd: Option<&str>,
    watch_cmd: Option<&str>,
) {
    // Sliders always expose read and write commands, so those are always listed
    commands.push(CommandReference {
        slot: format!("{base_slot}.get_cmd"),
        command: get_cmd.to_string(),
    });
    commands.push(CommandReference {
        slot: format!("{base_slot}.set_cmd"),
        command: set_cmd.to_string(),
    });
    push_optional_command(commands, &format!("{base_slot}.toggle_cmd"), toggle_cmd);
    push_optional_command(commands, &format!("{base_slot}.watch_cmd"), watch_cmd);
}

fn push_optional_command(commands: &mut Vec<CommandReference>, slot: &str, value: Option<&str>) {
    let Some(command) = value else {
        return;
    };
    let trimmed = command.trim();
    if trimmed.is_empty() {
        // Blank values are treated the same as missing values in reports
        return;
    }

    commands.push(CommandReference {
        slot: slot.to_string(),
        command: trimmed.to_string(),
    });
}
