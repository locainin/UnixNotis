use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::hooks;

use super::super::parts::MediaCardLayoutParts;
use super::super::shell::MediaShellConfig;
use super::plan::{nav_cluster_spacing_px, ShellCompositionPlan};

pub(super) fn compose_card_shell(
    shell: &MediaShellConfig,
    plan: &ShellCompositionPlan,
    parts: &MediaCardLayoutParts,
    nav_prev: &gtk::Button,
    nav_next: &gtk::Button,
) {
    // The card root is always vertical once the shell starts assembling sections
    parts.card.root.set_orientation(gtk::Orientation::Vertical);
    parts.card.root.set_spacing(shell.content_spacing_px);

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

fn build_inline_strip(
    shell: &MediaShellConfig,
    plan: &ShellCompositionPlan,
    controls: &gtk::Box,
    nav_prev: &gtk::Button,
    nav_next: &gtk::Button,
) -> Option<gtk::Box> {
    let include_controls = plan.inline_controls;
    let include_nav = plan.inline_nav;
    if !include_controls && !include_nav {
        // No active inline widgets means the trailing strip should not exist at all
        return None;
    }

    // Inline shells need one trailing cluster so the text gap and nav gap do not fight each other
    let inline_strip = gtk::Box::new(
        gtk::Orientation::Horizontal,
        nav_cluster_spacing_px(include_controls, include_nav, shell),
    );
    // Keep this wrapper classless so css-check only has to model the real public hook nodes
    inline_strip.set_hexpand(false);
    inline_strip.set_halign(Align::End);
    inline_strip.set_valign(Align::Center);
    if include_controls {
        // The shared controls box is mounted directly to avoid one more wrapper layer
        inline_strip.append(controls);
    }
    if include_nav {
        inline_strip.append(&build_navigation_strip(shell, nav_prev, nav_next));
    }
    Some(inline_strip)
}

fn build_bottom_strip(
    shell: &MediaShellConfig,
    plan: &ShellCompositionPlan,
    controls: &gtk::Box,
    nav_prev: &gtk::Button,
    nav_next: &gtk::Button,
) -> Option<gtk::Box> {
    let include_controls = plan.bottom_controls;
    let include_nav = plan.bottom_nav;
    if !include_controls && !include_nav {
        // Bottom rails only exist when something is actually assigned there
        return None;
    }

    // Bottom strips keep transport and player switching grouped on the same visual rail
    let control_strip = build_shell_box(
        gtk::Orientation::Horizontal,
        nav_cluster_spacing_px(include_controls, include_nav, shell),
        hooks::media_shell::CONTROL_STRIP,
    );
    control_strip.set_halign(Align::Fill);
    if include_controls {
        // Bottom shells keep transport width visible before any nav gets appended
        control_strip.append(controls);
    }
    if include_nav {
        control_strip.append(&build_navigation_strip(shell, nav_prev, nav_next));
    }
    Some(control_strip)
}

fn build_side_rail(
    shell: &MediaShellConfig,
    plan: &ShellCompositionPlan,
    controls: &gtk::Box,
    nav_prev: &gtk::Button,
    nav_next: &gtk::Button,
) -> Option<gtk::Box> {
    let include_controls = plan.side_controls;
    let include_nav = plan.side_nav;
    if !include_controls && !include_nav {
        // Side rails disappear entirely when nothing is routed into them
        return None;
    }

    // Side rails keep dashboard-style shells narrow without changing the text lane rules
    let action_rail = build_shell_box(
        gtk::Orientation::Vertical,
        nav_cluster_spacing_px(include_controls, include_nav, shell),
        hooks::media_shell::ACTION_RAIL,
    );
    action_rail.set_halign(Align::End);
    if include_controls {
        // Controls stay first so the nav pair reads as the secondary action group
        action_rail.append(controls);
    }
    if include_nav {
        action_rail.append(&build_navigation_strip(shell, nav_prev, nav_next));
    }
    Some(action_rail)
}

fn build_navigation_strip(
    shell: &MediaShellConfig,
    nav_prev: &gtk::Button,
    nav_next: &gtk::Button,
) -> gtk::Box {
    // A shared strip keeps inline, bottom, and side navigation on one spacing rule
    let nav_strip = build_shell_box(
        gtk::Orientation::Horizontal,
        shell.navigation_spacing_px,
        hooks::media_shell::NAV_STRIP,
    );
    nav_strip.set_halign(Align::Center);
    nav_strip.append(nav_prev);
    nav_strip.append(nav_next);
    nav_strip
}

fn build_shell_box(orientation: gtk::Orientation, spacing: i32, class_name: &str) -> gtk::Box {
    // Every structural shell box gets the same fill behavior so css only decides the look
    let shell_box = gtk::Box::new(orientation, spacing);
    shell_box.add_css_class(class_name);
    shell_box.set_hexpand(true);
    shell_box.set_halign(Align::Fill);
    shell_box.set_valign(Align::Center);
    shell_box
}
