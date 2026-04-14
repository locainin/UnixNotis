use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::media::MediaInfo;

use super::super::marquee::MarqueeLabel;
use super::super::media_art::apply_media_art;
use unixnotis_core::hooks;

#[derive(Clone)]
pub(super) struct MediaCardWidgets {
    pub(super) root: gtk::Box,
    pub(super) art: gtk::Picture,
    pub(super) text_box: gtk::Box,
    pub(super) meta_row: gtk::Box,
    pub(super) source_label: gtk::Label,
    pub(super) position_label: gtk::Label,
    pub(super) title_label: MarqueeLabel,
    pub(super) artist_label: gtk::Label,
    pub(super) play_button: gtk::Button,
    pub(super) next_button: gtk::Button,
    pub(super) prev_button: gtk::Button,
    pub(super) art_key: Rc<RefCell<Option<String>>>,
    pub(super) show_source_pref: Rc<Cell<bool>>,
    pub(super) show_position_pref: Rc<Cell<bool>>,
    pub(super) player_total: Rc<Cell<usize>>,
}

impl MediaCardWidgets {
    pub(super) fn apply_metadata_visibility(&self, show_source: bool, show_position: bool) {
        // Keep config intent cached so live player count can refine visibility later
        self.show_source_pref.set(show_source);
        self.show_position_pref.set(show_position);
        self.sync_metadata_visibility();
    }

    pub(super) fn update(&self, info: &MediaInfo, current: usize, total: usize) {
        // Skip text work when the value already matches the visible label
        if self.source_label.text() != info.identity.as_str() {
            self.source_label.set_text(&info.identity);
        }
        let position = format!("{current}/{total}");
        if self.position_label.text() != position.as_str() {
            self.position_label.set_text(&position);
        }
        self.player_total.set(total);
        self.sync_metadata_visibility();

        let title = if info.title.is_empty() {
            // Missing titles fall back to the player name instead of a blank line
            info.identity.clone()
        } else {
            info.title.clone()
        };
        // The marquee handles its own internal caching
        self.title_label.set_text(&title);
        update_artist_label(&self.artist_label, &info.artist);
        update_artist_classes(&self.root, &info.artist);
        update_play_button(&self.play_button, &info.playback_status);
        update_control_sensitivity(self, info);

        // Artwork loading is centralized so remote and local sources share one safety path
        apply_media_art(&self.art, &self.art_key, info.art_source.as_ref());
        update_art_classes(&self.root, info.art_source.is_some());
        update_player_count_classes(&self.root, total);
        if !self.art.is_visible() {
            self.art.set_visible(true);
        }

        update_playing_class(&self.root, &info.playback_status);
    }

    fn sync_metadata_visibility(&self) {
        // The source follows config directly
        let show_source = self.show_source_pref.get();
        self.source_label.set_visible(show_source);

        // The position badge only matters when more than one player can be cycled
        let show_position = self.show_position_pref.get() && self.player_total.get() > 1;
        self.position_label.set_visible(show_position);
        self.meta_row.set_visible(show_source || show_position);
    }
}

fn update_artist_label(label: &gtk::Label, artist: &str) {
    if artist.is_empty() {
        // A blank placeholder keeps the card height from jumping
        if label.text() != " " {
            label.set_text(" ");
        }
        if !label.has_css_class(hooks::shared_state::EMPTY) {
            label.add_css_class(hooks::shared_state::EMPTY);
        }
    } else {
        if label.text() != artist {
            label.set_text(artist);
        }
        if label.has_css_class(hooks::shared_state::EMPTY) {
            label.remove_css_class(hooks::shared_state::EMPTY);
        }
    }
    label.set_visible(true);
}

fn update_artist_classes(root: &gtk::Box, artist: &str) {
    let has_artist = !artist.is_empty();
    set_class_state(root, hooks::media_card::HAS_ARTIST, has_artist);
    set_class_state(root, hooks::media_card::EMPTY_ARTIST, !has_artist);
}
fn update_play_button(button: &gtk::Button, playback_status: &str) {
    let icon_name = if playback_status == "Playing" {
        "media-playback-pause-symbolic"
    } else {
        "media-playback-start-symbolic"
    };
    // Skip icon churn when playback state has not changed
    if button.icon_name().as_deref() != Some(icon_name) {
        button.set_icon_name(icon_name);
    }
}

fn update_control_sensitivity(card: &MediaCardWidgets, info: &MediaInfo) {
    // Each flag comes from MPRIS capabilities, so the UI mirrors player support directly
    let can_play = info.can_play || info.can_pause;
    if card.play_button.is_sensitive() != can_play {
        card.play_button.set_sensitive(can_play);
    }
    if card.next_button.is_sensitive() != info.can_next {
        card.next_button.set_sensitive(info.can_next);
    }
    if card.prev_button.is_sensitive() != info.can_prev {
        card.prev_button.set_sensitive(info.can_prev);
    }
}

fn update_playing_class(root: &gtk::Box, playback_status: &str) {
    if playback_status == "Playing" {
        // The css class drives the active glow only while playback is live
        if !root.has_css_class(hooks::shared_state::PLAYING) {
            root.add_css_class(hooks::shared_state::PLAYING);
        }
    } else if root.has_css_class(hooks::shared_state::PLAYING) {
        root.remove_css_class(hooks::shared_state::PLAYING);
    }

    set_class_state(
        root,
        hooks::media_card::PLAYING,
        playback_status == "Playing",
    );
    set_class_state(root, hooks::media_card::PAUSED, playback_status == "Paused");
    set_class_state(
        root,
        hooks::media_card::STOPPED,
        playback_status == "Stopped",
    );
}

fn update_art_classes(root: &gtk::Box, has_art: bool) {
    set_class_state(root, hooks::media_card::HAS_ART, has_art);
    set_class_state(root, hooks::media_card::NO_ART, !has_art);
}

fn update_player_count_classes(root: &gtk::Box, total: usize) {
    set_class_state(root, hooks::media_card::MULTI_PLAYER, total > 1);
    set_class_state(root, hooks::media_card::SINGLE_PLAYER, total <= 1);
}

fn set_class_state(root: &gtk::Box, class_name: &str, enabled: bool) {
    if enabled {
        if !root.has_css_class(class_name) {
            root.add_css_class(class_name);
        }
    } else if root.has_css_class(class_name) {
        root.remove_css_class(class_name);
    }
}
