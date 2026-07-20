use unixnotis_core::{CommandSpec, Config};

use super::CommandReference;

pub fn collect_command_references_from_config(config: &Config) -> Vec<CommandReference> {
    let mut commands = Vec::new();

    // Each widget family is collected separately so later checks can reason about real slot names
    collect_slider_commands(
        &mut commands,
        "widgets.volume",
        &config.widgets.volume.get_cmd,
        &config.widgets.volume.set_cmd,
        config.widgets.volume.toggle_cmd.as_ref(),
        config.widgets.volume.watch_cmd.as_ref(),
    );
    collect_slider_commands(
        &mut commands,
        "widgets.brightness",
        &config.widgets.brightness.get_cmd,
        &config.widgets.brightness.set_cmd,
        config.widgets.brightness.toggle_cmd.as_ref(),
        config.widgets.brightness.watch_cmd.as_ref(),
    );
    for (index, toggle) in config.widgets.toggles.iter().enumerate() {
        push_optional_command(
            &mut commands,
            &format!("widgets.toggles[{index}].state_cmd"),
            toggle.state_cmd.as_ref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.toggles[{index}].toggle_cmd"),
            toggle.toggle_cmd.as_ref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.toggles[{index}].on_cmd"),
            toggle.on_cmd.as_ref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.toggles[{index}].off_cmd"),
            toggle.off_cmd.as_ref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.toggles[{index}].watch_cmd"),
            toggle.watch_cmd.as_ref(),
        );
    }
    for (index, stat) in config.widgets.stats.iter().enumerate() {
        push_optional_command(
            &mut commands,
            &format!("widgets.stats[{index}].cmd"),
            stat.cmd.as_ref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.stats[{index}].plugin.command"),
            stat.plugin.as_ref().map(|plugin| &plugin.command),
        );
    }
    for (index, card) in config.widgets.cards.iter().enumerate() {
        push_optional_command(
            &mut commands,
            &format!("widgets.cards[{index}].cmd"),
            card.cmd.as_ref(),
        );
        push_optional_command(
            &mut commands,
            &format!("widgets.cards[{index}].plugin.command"),
            card.plugin.as_ref().map(|plugin| &plugin.command),
        );
    }

    commands
}

fn collect_slider_commands(
    commands: &mut Vec<CommandReference>,
    base_slot: &str,
    get_cmd: &CommandSpec,
    set_cmd: &CommandSpec,
    toggle_cmd: Option<&CommandSpec>,
    watch_cmd: Option<&CommandSpec>,
) {
    // Sliders always expose read and write commands, so those are always listed
    commands.push(CommandReference {
        slot: format!("{base_slot}.get_cmd"),
        command: get_cmd.clone(),
    });
    commands.push(CommandReference {
        slot: format!("{base_slot}.set_cmd"),
        command: set_cmd.clone(),
    });
    push_optional_command(commands, &format!("{base_slot}.toggle_cmd"), toggle_cmd);
    push_optional_command(commands, &format!("{base_slot}.watch_cmd"), watch_cmd);
}

fn push_optional_command(
    commands: &mut Vec<CommandReference>,
    slot: &str,
    value: Option<&CommandSpec>,
) {
    let Some(command) = value else {
        return;
    };
    if command.is_empty() {
        // Blank values are treated the same as missing values in reports
        return;
    }

    commands.push(CommandReference {
        slot: slot.to_string(),
        command: command.clone(),
    });
}
