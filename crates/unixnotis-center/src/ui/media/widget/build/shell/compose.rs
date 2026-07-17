use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::hooks;

use super::super::super::parts::MediaCardLayoutParts;
use super::super::super::shell::MediaShellConfig;
use super::super::plan::ShellCompositionPlan;
use super::alignment::{apply_player_layout_alignment, uses_centered_player_layout};
use super::strips::{build_bottom_strip, build_inline_strip, build_shell_box, build_side_rail};

pub(in super::super) fn compose_card_shell(
    shell: &MediaShellConfig,
    plan: &ShellCompositionPlan,
    parts: &MediaCardLayoutParts,
    nav_prev: &gtk::Button,
    nav_next: &gtk::Button,
) {
    // The card root is always vertical once the shell starts assembling sections
    parts.card.root.set_orientation(gtk::Orientation::Vertical);
    parts.card.root.set_spacing(shell.content_spacing_px);
    apply_player_layout_alignment(shell, plan, parts);

    if plan.top_art {
        // Top art gets its own row so wide text and side rails stay independent below it
        parts.card.root.append(&parts.art_frame);
    }

    let header = build_shell_box(
        gtk::Orientation::Horizontal,
        shell.content_spacing_px,
        hooks::media_shell::HEADER,
    );
    let main = build_shell_box(
        gtk::Orientation::Vertical,
        shell.content_spacing_px,
        hooks::media_shell::MAIN,
    );
    let body = build_shell_box(
        gtk::Orientation::Horizontal,
        shell.content_spacing_px,
        hooks::media_shell::BODY,
    );
    if uses_centered_player_layout(shell, plan) {
        // The dedicated player preset centers the text lane under the cover art
        body.set_hexpand(false);
        body.set_halign(Align::Center);
    }

    // Text stays as the left-side anchor for every shell variant
    body.append(&parts.card.text_box);
    if let Some(inline_strip) = build_inline_strip(shell, plan, &parts.controls, nav_prev, nav_next)
    {
        // Keep text spacing independent from the inner spacing between controls and nav
        body.append(&inline_strip);
    }
    if let Some(side_rail) = build_side_rail(shell, plan, &parts.controls, nav_prev, nav_next) {
        body.append(&side_rail);
    }

    // Main owns the text lane and any lower control rail that follows it
    main.append(&body);
    if let Some(bottom_strip) = build_bottom_strip(shell, plan, &parts.controls, nav_prev, nav_next)
    {
        main.append(&bottom_strip);
    }

    // Header is the shared top-level row whether art sits beside or above the main section
    if plan.start_art {
        header.append(&parts.art_frame);
    }
    header.append(&main);
    parts.card.root.append(&header);
}
