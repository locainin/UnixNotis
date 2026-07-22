//! Panel-local motion preference styling

use gtk::prelude::*;
use unixnotis_core::css::hooks;

pub(in crate::ui) fn apply_reduced_motion(root: &gtk::Box, reduced_motion: bool) {
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
