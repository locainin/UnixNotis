//! Ghost row widget construction and updates.

use std::cell::RefCell;

use gtk::prelude::*;
use unixnotis_core::css::hooks;

use super::list_item::RowData;

pub(super) struct GhostRowWidgets {
    pub(super) depth: RefCell<u8>,
}

pub(super) fn build_ghost_row() -> (gtk::Box, GhostRowWidgets) {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("unixnotis-panel-card");
    root.add_css_class(hooks::ghost_row::ROOT);
    root.set_visible(true);

    (
        root,
        GhostRowWidgets {
            depth: RefCell::new(0),
        },
    )
}

pub(super) fn update_ghost_row(ghost: &GhostRowWidgets, root: &gtk::Box, data: &RowData) {
    let mut depth = ghost.depth.borrow_mut();
    if *depth == data.ghost_depth {
        return;
    }
    if *depth > 0 {
        root.remove_css_class(&format!("{}{}", hooks::ghost_row::DEPTH_PREFIX, *depth));
    }
    if data.ghost_depth > 0 {
        root.add_css_class(&format!(
            "{}{}",
            hooks::ghost_row::DEPTH_PREFIX,
            data.ghost_depth
        ));
    }
    *depth = data.ghost_depth;
}
