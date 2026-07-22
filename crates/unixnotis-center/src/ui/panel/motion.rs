//! Panel-local motion preference policy

use gtk::prelude::*;
use unixnotis_core::css::hooks;

use super::body::WIDGET_REVEAL_TRANSITION_MS;
use super::header::search::SEARCH_REVEAL_TRANSITION_MS;
use super::notice::RELOAD_NOTICE_TRANSITION_MS;
use super::widgets::PanelWidgets;
use crate::ui::motion::apply_revealer_preference;

pub(in crate::ui) fn apply_reduced_motion(panel: &PanelWidgets, reduced_motion: bool) {
    apply_motion_class(&panel.root, reduced_motion);
    apply_revealer_preference(
        &panel.sections.widget_revealer,
        WIDGET_REVEAL_TRANSITION_MS as u32,
        reduced_motion,
    );
    apply_revealer_preference(
        &panel.header.search.revealer,
        SEARCH_REVEAL_TRANSITION_MS as u32,
        reduced_motion,
    );
    apply_revealer_preference(
        &panel.reload_notice.revealer,
        RELOAD_NOTICE_TRANSITION_MS,
        reduced_motion,
    );
}

fn apply_motion_class(root: &gtk::Box, reduced_motion: bool) {
    if reduced_motion {
        // One stable class lets the internal policy layer cover custom and stock themes
        root.add_css_class(hooks::panel_shell::REDUCED_MOTION);
    } else {
        root.remove_css_class(hooks::panel_shell::REDUCED_MOTION);
    }
}

#[cfg(test)]
#[path = "tests/motion.rs"]
mod tests;
