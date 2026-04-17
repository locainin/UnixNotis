use gtk::prelude::*;
use gtk::Align;

use super::super::super::parts::MediaCardLayoutParts;
use super::super::super::shell::MediaShellConfig;
use super::super::plan::ShellCompositionPlan;

pub(super) fn apply_player_layout_alignment(
    shell: &MediaShellConfig,
    plan: &ShellCompositionPlan,
    parts: &MediaCardLayoutParts,
) {
    if !uses_centered_player_layout(shell, plan) {
        return;
    }

    // The player preset shares one center line across art, text, and the transport dock
    parts.card.text_box.set_hexpand(false);
    parts.card.text_box.set_halign(Align::Center);
    parts.card.meta_row.set_halign(Align::Center);
    parts.card.title_widget.set_halign(Align::Center);
    parts.card.artist_label.set_xalign(0.5);
    parts.card.source_label.set_xalign(0.5);
    parts.card.position_label.set_xalign(0.5);

    // The transport cluster should sit directly under the cover art instead of hugging the edge
    parts.controls.set_halign(Align::Center);
}

pub(super) fn uses_centered_player_layout(
    shell: &MediaShellConfig,
    plan: &ShellCompositionPlan,
) -> bool {
    shell.layout == unixnotis_core::MediaLayout::Player && plan.top_art
}
