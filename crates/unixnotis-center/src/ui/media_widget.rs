//! Media carousel widget for the center panel.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, Align};

use crate::media::{MediaHandle, MediaInfo};

use super::marquee::MarqueeLabel;

/// GTK widget that renders media players with an in-panel carousel.
pub struct MediaWidget {
    root: gtk::Box,
    nav_prev: gtk::Button,
    nav_next: gtk::Button,
    card: MediaCardWidgets,
    selection: Rc<RefCell<MediaSelection>>,
}

#[derive(Default)]
struct MediaSelection {
    players: Vec<MediaInfo>,
    current_index: usize,
}

#[derive(Clone)]
struct MediaCardWidgets {
    root: gtk::Box,
    art: gtk::Picture,
    text_box: gtk::Box,
    source_label: gtk::Label,
    position_label: gtk::Label,
    title_label: MarqueeLabel,
    artist_label: gtk::Label,
    play_button: gtk::Button,
    next_button: gtk::Button,
    prev_button: gtk::Button,
    art_uri: Rc<RefCell<Option<String>>>,
}

impl MediaWidget {
    pub fn new(
        container: &gtk::Box,
        handle: MediaHandle,
        panel_width: i32,
        title_char_limit: usize,
    ) -> Self {
        // Reserve space for art, controls, and padding to keep the title scroller fixed.
        let marquee_width = panel_width.saturating_sub(240).max(140);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.add_css_class("unixnotis-media-stack");
        root.set_visible(false);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.add_css_class("unixnotis-media-row");
        row.set_hexpand(true);

        let nav_prev = gtk::Button::with_label("<");
        nav_prev.add_css_class("unixnotis-media-nav");

        let nav_next = gtk::Button::with_label(">");
        nav_next.add_css_class("unixnotis-media-nav");

        let selection = Rc::new(RefCell::new(MediaSelection::default()));
        let card = build_media_card(&handle, selection.clone(), marquee_width, title_char_limit);

        row.append(&nav_prev);
        row.append(&card.root);
        row.append(&nav_next);
        root.append(&row);
        container.append(&root);

        let selection_prev = selection.clone();
        let card_prev = card.clone();
        let root_prev = root.clone();
        let nav_prev_clone = nav_prev.clone();
        let nav_next_clone = nav_next.clone();
        nav_prev.connect_clicked(move |_| {
            selection_prev.borrow_mut().prev();
            apply_selection(
                &selection_prev.borrow(),
                &card_prev,
                &root_prev,
                &nav_prev_clone,
                &nav_next_clone,
            );
        });

        let selection_next = selection.clone();
        let card_next = card.clone();
        let root_next = root.clone();
        let nav_prev_clone = nav_prev.clone();
        let nav_next_clone = nav_next.clone();
        nav_next.connect_clicked(move |_| {
            selection_next.borrow_mut().next();
            apply_selection(
                &selection_next.borrow(),
                &card_next,
                &root_next,
                &nav_prev_clone,
                &nav_next_clone,
            );
        });

        Self {
            root,
            nav_prev,
            nav_next,
            card,
            selection,
        }
    }

    pub fn update(&mut self, infos: &[MediaInfo]) {
        self.selection.borrow_mut().set_players(infos.to_vec());
        apply_selection(
            &self.selection.borrow(),
            &self.card,
            &self.root,
            &self.nav_prev,
            &self.nav_next,
        );
    }

    pub fn clear(&mut self) {
        self.selection.borrow_mut().players.clear();
        self.root.set_visible(false);
    }

    pub fn apply_layout(&mut self, panel_width: i32, title_char_limit: usize) {
        let marquee_width = panel_width.saturating_sub(240).max(140);
        self.card.text_box.set_size_request(marquee_width, -1);
        self.card
            .title_label
            .update_limits(marquee_width, title_char_limit);
    }
}

impl MediaSelection {
    fn set_players(&mut self, players: Vec<MediaInfo>) {
        let current_bus = self.current_bus();
        self.players = players;
        if self.players.is_empty() {
            self.current_index = 0;
            return;
        }
        if let Some(current_bus) = current_bus {
            if let Some(index) = self
                .players
                .iter()
                .position(|info| info.bus_name == current_bus)
            {
                self.current_index = index;
                return;
            }
        }
        self.current_index = 0;
    }

    fn current(&self) -> Option<&MediaInfo> {
        self.players.get(self.current_index)
    }

    fn current_bus(&self) -> Option<String> {
        self.current().map(|info| info.bus_name.clone())
    }

    fn next(&mut self) {
        if self.players.len() <= 1 {
            return;
        }
        self.current_index = (self.current_index + 1) % self.players.len();
    }

    fn prev(&mut self) {
        if self.players.len() <= 1 {
            return;
        }
        if self.current_index == 0 {
            self.current_index = self.players.len() - 1;
        } else {
            self.current_index -= 1;
        }
    }

    fn has_multiple(&self) -> bool {
        self.players.len() > 1
    }

    fn position(&self) -> (usize, usize) {
        if self.players.is_empty() {
            return (0, 0);
        }
        (self.current_index + 1, self.players.len())
    }
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
        card.update(info, current, total);
        root.set_visible(true);
    } else {
        root.set_visible(false);
    }

    let has_multiple = selection.has_multiple();
    nav_prev.set_sensitive(has_multiple);
    nav_next.set_sensitive(has_multiple);
}

impl MediaCardWidgets {
    fn update(&self, info: &MediaInfo, current: usize, total: usize) {
        self.source_label.set_text(&info.identity);
        self.position_label.set_text(&format!("{current}/{total}"));

        let title = if info.title.is_empty() {
            info.identity.clone()
        } else {
            info.title.clone()
        };
        self.title_label.set_text(&title);

        if info.artist.is_empty() {
            self.artist_label.set_text(" ");
            self.artist_label.add_css_class("empty");
        } else {
            self.artist_label.set_text(&info.artist);
            self.artist_label.remove_css_class("empty");
        }
        self.artist_label.set_visible(true);

        let icon_name = if info.playback_status == "Playing" {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };
        self.play_button.set_icon_name(icon_name);

        self.play_button
            .set_sensitive(info.can_play || info.can_pause);
        self.next_button.set_sensitive(info.can_next);
        self.prev_button.set_sensitive(info.can_prev);

        let next_uri = info.art_uri.clone();
        if *self.art_uri.borrow() != next_uri {
            if let Some(uri) = info.art_uri.as_ref() {
                let file = if uri.starts_with("http://")
                    || uri.starts_with("https://")
                    || uri.starts_with("file://")
                {
                    gio::File::for_uri(uri)
                } else {
                    gio::File::for_path(uri)
                };
                self.art.set_file(Some(&file));
                self.art.remove_css_class("empty");
            } else {
                self.art.set_file(None::<&gio::File>);
                self.art.add_css_class("empty");
            }
            *self.art_uri.borrow_mut() = next_uri;
        }
        self.art.set_visible(true);

        if info.playback_status == "Playing" {
            self.root.add_css_class("playing");
        } else {
            self.root.remove_css_class("playing");
        }
    }
}

fn build_media_card(
    handle: &MediaHandle,
    selection: Rc<RefCell<MediaSelection>>,
    marquee_width: i32,
    title_char_limit: usize,
) -> MediaCardWidgets {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    root.add_css_class("unixnotis-media-card");
    root.set_hexpand(true);
    root.set_halign(Align::Fill);
    root.set_valign(Align::Center);
    // Fixed height keeps the media pill consistent across metadata variants.
    root.set_size_request(-1, 72);

    let art = gtk::Picture::new();
    art.add_css_class("unixnotis-media-art");
    art.set_can_shrink(true);
    art.set_size_request(50, 50);
    art.set_keep_aspect_ratio(true);
    art.set_hexpand(false);
    art.set_vexpand(false);
    art.set_halign(Align::Center);
    art.set_valign(Align::Center);
    art.set_visible(false);

    let art_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    art_frame.add_css_class("unixnotis-media-art-frame");
    art_frame.set_size_request(54, 54);
    art_frame.set_hexpand(false);
    art_frame.set_vexpand(false);
    art_frame.set_halign(Align::Center);
    art_frame.set_valign(Align::Center);
    art_frame.append(&art);

    let info_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    info_row.set_hexpand(true);
    info_row.set_halign(Align::Fill);
    info_row.set_valign(Align::Center);

    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text_box.set_hexpand(false);
    text_box.set_halign(Align::Fill);
    text_box.set_size_request(marquee_width, -1);

    let source_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let source_label = gtk::Label::new(Some(""));
    source_label.set_xalign(0.0);
    source_label.add_css_class("unixnotis-media-source");

    let position_label = gtk::Label::new(Some(""));
    position_label.set_xalign(1.0);
    position_label.add_css_class("unixnotis-media-position");

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    spacer.set_hexpand(true);

    source_row.append(&source_label);
    source_row.append(&spacer);
    source_row.append(&position_label);

    let title_label = MarqueeLabel::new("unixnotis-media-title", marquee_width, title_char_limit);
    let marquee_widget = title_label.widget();
    marquee_widget.set_hexpand(false);
    marquee_widget.set_halign(Align::Start);

    let artist_label = gtk::Label::new(None);
    artist_label.set_xalign(0.0);
    artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    artist_label.add_css_class("unixnotis-media-artist");

    text_box.append(&source_row);
    text_box.append(&marquee_widget);
    text_box.append(&artist_label);

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    controls.add_css_class("unixnotis-media-controls");
    controls.set_halign(Align::End);
    controls.set_valign(Align::Center);

    let prev_button = gtk::Button::from_icon_name("media-skip-backward-symbolic");
    let play_button = gtk::Button::from_icon_name("media-playback-start-symbolic");
    let next_button = gtk::Button::from_icon_name("media-skip-forward-symbolic");

    prev_button.add_css_class("unixnotis-media-button");
    play_button.add_css_class("unixnotis-media-button");
    play_button.add_css_class("primary");
    next_button.add_css_class("unixnotis-media-button");

    controls.append(&prev_button);
    controls.append(&play_button);
    controls.append(&next_button);

    info_row.append(&text_box);
    info_row.append(&controls);

    root.append(&art_frame);
    root.append(&info_row);

    let selection_play = selection.clone();
    let handle_play = handle.clone();
    play_button.connect_clicked(move |_| {
        if let Some(bus_name) = selection_play.borrow().current_bus() {
            handle_play.play_pause(&bus_name);
        }
    });

    let selection_next = selection.clone();
    let handle_next = handle.clone();
    next_button.connect_clicked(move |_| {
        if let Some(bus_name) = selection_next.borrow().current_bus() {
            handle_next.next(&bus_name);
        }
    });

    let selection_prev = selection;
    let handle_prev = handle.clone();
    prev_button.connect_clicked(move |_| {
        if let Some(bus_name) = selection_prev.borrow().current_bus() {
            handle_prev.previous(&bus_name);
        }
    });

    let art_uri = Rc::new(RefCell::new(None));

    MediaCardWidgets {
        root,
        art,
        text_box,
        source_label,
        position_label,
        title_label,
        artist_label,
        play_button,
        next_button,
        prev_button,
        art_uri,
    }
}
