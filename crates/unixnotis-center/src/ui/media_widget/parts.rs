use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{Align, Overflow};

use crate::media::MediaHandle;
use crate::ui::input_guard::ClickCooldown;

use super::super::marquee::MarqueeLabel;
use super::super::media_art::MediaArtState;
use super::card::MediaCardWidgets;
use super::format::MediaDisplayConfig;
use super::selection::MediaSelection;
use super::shell::MediaShellConfig;
use unixnotis_core::{hooks, MediaConfig};

const MEDIA_CLICK_GUARD_MS: u64 = 120;

pub(super) struct MediaCardLayoutParts {
    pub(super) card: MediaCardWidgets,
    pub(super) art_frame: gtk::Box,
    pub(super) controls: gtk::Box,
}

pub(super) fn build_media_card_parts(
    handle: &MediaHandle,
    selection: Rc<RefCell<MediaSelection>>,
    marquee_width: i32,
    config: &MediaConfig,
    shell: &MediaShellConfig,
) -> MediaCardLayoutParts {
    // The shared card starts neutral and gets arranged later by each layout shell
    let root = gtk::Box::new(gtk::Orientation::Vertical, shell.content_spacing_px);
    root.add_css_class(hooks::media_shell::CARD);
    root.set_hexpand(true);
    root.set_halign(Align::Fill);
    root.set_valign(Align::Center);

    let art = build_art_picture(shell.art_size_px);
    // The frame owns the visible slot size even when artwork is missing
    let art_frame = build_art_frame(&art, shell.art_size_px.saturating_add(4));

    let text_box = build_text_box(marquee_width);
    // The meta row keeps source and counter grouped so one visibility toggle can hide both
    let meta_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    meta_row.add_css_class(hooks::media_shell::META);
    meta_row.set_hexpand(true);
    meta_row.set_halign(Align::Fill);

    let source_label = gtk::Label::new(Some(""));
    source_label.set_xalign(0.0);
    source_label.add_css_class(hooks::media_shell::SOURCE);

    let position_label = gtk::Label::new(Some(""));
    position_label.set_xalign(1.0);
    position_label.add_css_class(hooks::media_shell::POSITION);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    spacer.set_hexpand(true);
    // The spacer keeps source left-aligned and the counter pushed to the far edge
    meta_row.append(&source_label);
    meta_row.append(&spacer);
    meta_row.append(&position_label);

    // The marquee widget owns title truncation and scrolling rules in one place
    let title_label = MarqueeLabel::new(
        hooks::media_shell::TITLE,
        marquee_width,
        config.title_char_limit,
    );
    let title_widget = title_label.widget();
    title_widget.set_hexpand(false);
    title_widget.set_halign(Align::Start);

    let artist_label = gtk::Label::new(None);
    artist_label.set_xalign(0.0);
    artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    artist_label.add_css_class(hooks::media_shell::ARTIST);

    text_box.append(&meta_row);
    text_box.append(&title_widget);
    text_box.append(&artist_label);

    // Transport buttons are shared across every shell preset
    let (controls, prev_button, play_button, next_button) =
        build_controls(shell.control_spacing_px);
    connect_playback_buttons(handle, selection, &play_button, &next_button, &prev_button);

    // The art key lets async art loads ignore stale completions
    let art_state = Rc::new(RefCell::new(MediaArtState::default()));
    // Display config stays shared so reloads can re-render the visible player in place
    let display = Rc::new(RefCell::new(MediaDisplayConfig::from_config(config)));
    let player_total = Rc::new(Cell::new(0usize));

    MediaCardLayoutParts {
        art_frame,
        controls,
        card: MediaCardWidgets {
            root,
            art,
            text_box,
            meta_row,
            source_label,
            position_label,
            title_widget,
            title_label,
            artist_label,
            play_button,
            next_button,
            prev_button,
            art_state,
            display,
            player_total,
        },
    }
}

fn build_art_picture(art_size_px: i32) -> gtk::Picture {
    // Artwork starts hidden so empty players do not leave a blank image box
    let art = gtk::Picture::new();
    art.add_css_class(hooks::media_shell::ART);
    art.set_can_shrink(true);
    art.set_size_request(art_size_px, art_size_px);
    art.set_keep_aspect_ratio(true);
    art.set_hexpand(false);
    art.set_vexpand(false);
    art.set_halign(Align::Center);
    art.set_valign(Align::Center);
    art.set_overflow(Overflow::Hidden);
    art.set_visible(false);
    art
}

fn build_art_frame(art: &gtk::Picture, frame_size_px: i32) -> gtk::Box {
    // The frame keeps the slot size stable even when the art widget is hidden
    let art_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    art_frame.add_css_class(hooks::media_shell::ART_FRAME);
    art_frame.set_size_request(frame_size_px, frame_size_px);
    art_frame.set_hexpand(false);
    art_frame.set_vexpand(false);
    art_frame.set_halign(Align::Center);
    art_frame.set_valign(Align::Center);
    art_frame.set_overflow(Overflow::Hidden);
    art_frame.append(art);
    art_frame
}

fn build_text_box(marquee_width: i32) -> gtk::Box {
    // The title lane gets an explicit width so relayout math stays predictable
    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text_box.add_css_class(hooks::media_shell::TEXT);
    text_box.set_hexpand(false);
    text_box.set_halign(Align::Fill);
    text_box.set_valign(Align::Center);
    text_box.set_size_request(marquee_width, -1);
    text_box
}

fn build_controls(spacing_px: i32) -> (gtk::Box, gtk::Button, gtk::Button, gtk::Button) {
    // Controls stay grouped so shells can move them as one block
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, spacing_px);
    controls.add_css_class(hooks::media_shell::CONTROLS);
    controls.set_halign(Align::End);
    controls.set_valign(Align::Center);

    let prev_button = gtk::Button::from_icon_name("media-skip-backward-symbolic");
    let play_button = gtk::Button::from_icon_name("media-playback-start-symbolic");
    let next_button = gtk::Button::from_icon_name("media-skip-forward-symbolic");

    prev_button.add_css_class(hooks::media_shell::BUTTON);
    prev_button.add_css_class(hooks::media_shell::BUTTON_PREV);
    play_button.add_css_class(hooks::media_shell::BUTTON);
    play_button.add_css_class(hooks::media_shell::BUTTON_PLAY);
    // The primary class gives themes one hook for the play or pause button
    play_button.add_css_class("primary");
    next_button.add_css_class(hooks::media_shell::BUTTON);
    next_button.add_css_class(hooks::media_shell::BUTTON_NEXT);

    controls.append(&prev_button);
    controls.append(&play_button);
    controls.append(&next_button);
    (controls, prev_button, play_button, next_button)
}

fn connect_playback_buttons(
    handle: &MediaHandle,
    selection: Rc<RefCell<MediaSelection>>,
    play_button: &gtk::Button,
    next_button: &gtk::Button,
    prev_button: &gtk::Button,
) {
    let selection_play = selection.clone();
    let handle_play = handle.clone();
    let play_gate = ClickCooldown::new(Duration::from_millis(MEDIA_CLICK_GUARD_MS));
    play_button.connect_clicked(move |_| {
        if !play_gate.try_start() {
            return;
        }
        // The current card decides which player receives transport commands
        if let Some(bus_name) = selection_play.borrow().current_bus() {
            handle_play.play_pause(&bus_name);
        }
    });

    let selection_next = selection.clone();
    let handle_next = handle.clone();
    let next_gate = ClickCooldown::new(Duration::from_millis(MEDIA_CLICK_GUARD_MS));
    next_button.connect_clicked(move |_| {
        if !next_gate.try_start() {
            return;
        }
        // Next only targets the currently selected player
        if let Some(bus_name) = selection_next.borrow().current_bus() {
            handle_next.next(&bus_name);
        }
    });

    let selection_prev = selection;
    let handle_prev = handle.clone();
    let prev_gate = ClickCooldown::new(Duration::from_millis(MEDIA_CLICK_GUARD_MS));
    prev_button.connect_clicked(move |_| {
        if !prev_gate.try_start() {
            return;
        }
        // Previous only targets the currently selected player
        if let Some(bus_name) = selection_prev.borrow().current_bus() {
            handle_prev.previous(&bus_name);
        }
    });
}
