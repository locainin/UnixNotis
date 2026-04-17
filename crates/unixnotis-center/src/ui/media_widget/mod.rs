mod build;
mod card;
mod format;
mod layout;
mod parts;
mod selection;
mod shell;

use std::cell::RefCell;
use std::rc::Rc;

use glib::clone;
use gtk::prelude::*;

use crate::media::{MediaHandle, MediaInfo};
use unixnotis_core::MediaConfig;

use self::build::build_media_widget;
use self::card::MediaCardWidgets;
use self::layout::marquee_width_for_shell;
use self::selection::{MediaSelection, MediaSelectionSnapshot};
use self::shell::MediaShellConfig;

pub struct MediaWidget {
    // The outer stack is what gets attached to the panel container
    root: gtk::Box,
    // Outer nav buttons are still used by the carousel shell and reused elsewhere
    nav_prev: gtk::Button,
    nav_next: gtk::Button,
    // The shared card owns labels, art, and transport buttons
    card: MediaCardWidgets,
    // Selection state is shared with button handlers and update calls
    selection: Rc<RefCell<MediaSelection>>,
    // The resolved shell captures structure and geometry that need a rebuild when changed
    shell: MediaShellConfig,
}

impl MediaWidget {
    pub(super) fn new(
        container: &gtk::Box,
        handle: MediaHandle,
        panel_width: i32,
        config: &MediaConfig,
    ) -> Self {
        let selection = Rc::new(RefCell::new(MediaSelection::default()));
        let shell = MediaShellConfig::from_config(config);
        // One build call assembles the shared card parts into the requested preset
        let built = build_media_widget(&handle, selection.clone(), panel_width, config, &shell);
        let root = built.root;
        let nav_prev = built.nav_prev;
        let nav_next = built.nav_next;
        let card = built.card;
        // The container only ever holds one media shell at a time
        container.append(&root);

        // Nav buttons update selection only, never the player list itself
        connect_prev_button(&nav_prev, &nav_next, &root, selection.clone(), card.clone());
        connect_next_button(&nav_prev, &nav_next, &root, selection.clone(), card.clone());

        Self {
            root,
            nav_prev,
            nav_next,
            card,
            selection,
            shell,
        }
    }

    pub(super) fn update(&mut self, infos: &[MediaInfo]) {
        // Snapshot replacement keeps the carousel in sync with the latest media list
        self.selection.borrow_mut().set_players(infos.to_vec());
        apply_selection(
            &self.selection.borrow(),
            &self.card,
            &self.root,
            &self.nav_prev,
            &self.nav_next,
        );
    }

    pub(super) fn clear(&mut self) {
        // Clearing hides the whole media stack so stale art never lingers
        self.selection.borrow_mut().players.clear();
        self.root.set_visible(false);
    }

    pub(super) fn matches_layout(&self, config: &MediaConfig) -> bool {
        self.shell == MediaShellConfig::from_config(config)
    }

    pub(super) fn snapshot(&self) -> MediaSelectionSnapshot {
        // Reload code uses this to rebuild the shell without losing context
        self.selection.borrow().snapshot()
    }

    pub(super) fn restore_snapshot(&mut self, snapshot: &MediaSelectionSnapshot) {
        // Layout rebuilds should preserve the visible player when the old bus still exists
        self.selection.borrow_mut().restore_snapshot(snapshot);
        apply_selection(
            &self.selection.borrow(),
            &self.card,
            &self.root,
            &self.nav_prev,
            &self.nav_next,
        );
    }

    pub(super) fn apply_layout(&mut self, panel_width: i32, config: &MediaConfig) {
        // Width updates stay lightweight when the structural preset is unchanged
        let marquee_width = marquee_width_for_shell(&self.shell, panel_width);
        // Metadata formatting and visibility are light enough to update in place
        self.card.apply_display_config(config);
        // Text width stays the only size request that changes on a light relayout
        self.card.text_box.set_size_request(marquee_width, -1);
        self.card
            .title_label
            .update_limits(marquee_width, config.title_char_limit);
        apply_selection(
            &self.selection.borrow(),
            &self.card,
            &self.root,
            &self.nav_prev,
            &self.nav_next,
        );
    }
}

fn connect_prev_button(
    nav_prev: &gtk::Button,
    nav_next: &gtk::Button,
    root: &gtk::Box,
    selection: Rc<RefCell<MediaSelection>>,
    card: MediaCardWidgets,
) {
    let selection_prev = selection.clone();
    let card_prev = card.clone();
    nav_prev.connect_clicked(clone!(
        #[weak]
        root,
        #[weak]
        nav_prev,
        #[weak]
        nav_next,
        #[strong]
        selection_prev,
        #[strong]
        card_prev,
        move |_| {
            // Weak captures avoid cycles when the panel rebuilds on config reload
            selection_prev.borrow_mut().prev();
            apply_selection(
                &selection_prev.borrow(),
                &card_prev,
                &root,
                &nav_prev,
                &nav_next,
            );
        }
    ));
}

fn connect_next_button(
    nav_prev: &gtk::Button,
    nav_next: &gtk::Button,
    root: &gtk::Box,
    selection: Rc<RefCell<MediaSelection>>,
    card: MediaCardWidgets,
) {
    let selection_next = selection;
    let card_next = card;
    nav_next.connect_clicked(clone!(
        #[weak]
        root,
        #[weak]
        nav_prev,
        #[weak]
        nav_next,
        #[strong]
        selection_next,
        #[strong]
        card_next,
        move |_| {
            // Weak captures avoid cycles when the panel rebuilds on config reload
            selection_next.borrow_mut().next();
            apply_selection(
                &selection_next.borrow(),
                &card_next,
                &root,
                &nav_prev,
                &nav_next,
            );
        }
    ));
}

fn apply_selection(
    selection: &MediaSelection,
    card: &MediaCardWidgets,
    root: &gtk::Box,
    nav_prev: &gtk::Button,
    nav_next: &gtk::Button,
) {
    if let Some(info) = selection.current() {
        let (current, total) = selection.position();
        // One update call refreshes labels, controls, and artwork together
        card.update(info, current, total);
        root.set_visible(true);
    } else {
        // No active player means the whole media shell should disappear
        root.set_visible(false);
    }

    let has_multiple = selection.has_multiple();
    // Single-player snapshots do not need navigation affordances
    nav_prev.set_sensitive(has_multiple);
    nav_next.set_sensitive(has_multiple);
}
