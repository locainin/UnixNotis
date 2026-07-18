use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{hooks, MediaConfig};

use crate::media::MediaInfo;
use crate::ui::media::artwork::MediaArtState;
use crate::ui::media::marquee::MarqueeLabel;

use super::super::format::MediaDisplayConfig;
use super::set_scrolled_content_width;
use super::{
    set_class_state, update_art_classes, update_artist_classes, update_artist_label,
    update_control_sensitivity, update_optional_label, update_play_button,
    update_player_count_classes, update_playing_class, update_title_label, MediaCardWidgets,
};

#[gtk::test]
fn scrolled_content_width_transitions_preserve_an_exact_valid_range() {
    let boundary = gtk::ScrolledWindow::new();
    set_scrolled_content_width(&boundary, 100);

    // Growing must raise GTK's maximum before its minimum
    set_scrolled_content_width(&boundary, 240);
    assert_eq!(boundary.min_content_width(), 240);
    assert_eq!(boundary.max_content_width(), 240);

    // Shrinking must lower GTK's minimum before its maximum
    set_scrolled_content_width(&boundary, 80);
    assert_eq!(boundary.min_content_width(), 80);
    assert_eq!(boundary.max_content_width(), 80);
}

fn card() -> MediaCardWidgets {
    let title_label = MarqueeLabel::new(hooks::media_shell::TITLE, 180, 32);
    let title_widget = title_label.widget();
    let art = gtk::Picture::new();
    let art_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    art_frame.append(&art);

    MediaCardWidgets {
        root: gtk::Box::new(gtk::Orientation::Vertical, 0),
        art,
        art_frame,
        text_box: gtk::Box::new(gtk::Orientation::Vertical, 0),
        meta_row: gtk::Box::new(gtk::Orientation::Horizontal, 0),
        source_label: gtk::Label::new(None),
        position_label: gtk::Label::new(None),
        title_widget,
        title_boundary: gtk::ScrolledWindow::new(),
        title_label,
        single_player_text_width: Cell::new(180),
        multi_player_text_width: Cell::new(120),
        applied_text_width: Cell::new(180),
        artist_label: gtk::Label::new(None),
        play_button: gtk::Button::new(),
        next_button: gtk::Button::new(),
        prev_button: gtk::Button::new(),
        art_state: Rc::new(RefCell::new(MediaArtState::default())),
        display: Rc::new(RefCell::new(MediaDisplayConfig::from_config(
            &MediaConfig::default(),
        ))),
        player_total: Rc::new(Cell::new(0)),
    }
}

fn media_info() -> MediaInfo {
    MediaInfo {
        bus_name: "org.mpris.MediaPlayer2.test".to_string(),
        identity: "Test Player".to_string(),
        browser_family: None,
        owner_pid: None,
        title: "A Track".to_string(),
        artist: "An Artist".to_string(),
        playback_status: "Playing".to_string(),
        art_source: None,
        can_play: true,
        can_pause: true,
        can_next: false,
        can_prev: true,
    }
}

#[gtk::test]
fn update_applies_metadata_capabilities_and_missing_art_state() {
    let card = card();

    card.update(&media_info(), 1, 1);

    assert_eq!(card.source_label.text(), "Test Player");
    assert_eq!(card.artist_label.text(), "An Artist");
    assert!(card.root.has_css_class(hooks::media_card::PLAYING));
    assert!(card.play_button.is_sensitive());
    assert!(card.prev_button.is_sensitive());
    assert!(!card.next_button.is_sensitive());
    assert!(card.art_frame.get_visible());
    assert!(!card.art.get_visible());
}

#[gtk::test]
fn consecutive_track_updates_replace_old_title_artist_and_source() {
    let card = card();
    let first = media_info();
    card.update(&first, 1, 1);

    let mut second = first;
    second.identity = "Other Player".to_string();
    second.title = "A Different Track".to_string();
    second.artist = "A Different Artist".to_string();
    card.update(&second, 1, 1);

    assert_eq!(card.source_label.text(), "Other Player");
    let rendered_title = card
        .title_widget
        .first_child()
        .and_downcast::<gtk::Label>()
        .expect("marquee title label");
    assert_eq!(rendered_title.text(), "A Different Track");
    assert_eq!(card.artist_label.text(), "A Different Artist");
}

#[gtk::test]
fn blank_artist_label_can_collapse_and_clears_stale_text() {
    let label = gtk::Label::new(Some("stale"));

    update_artist_label(&label, None);

    assert_eq!(label.text(), "");
    assert!(!label.get_visible());

    update_artist_label(&label, Some(" "));
    assert_eq!(label.text(), " ");
    assert!(label.has_css_class(hooks::shared_state::EMPTY));
    assert!(label.get_visible());
}

#[gtk::test]
fn optional_and_title_labels_clear_stale_content_and_restore_visibility() {
    let label = gtk::Label::new(Some("stale"));
    update_optional_label(&label, None);
    assert_eq!(label.text(), "");
    assert!(!label.get_visible());

    update_optional_label(&label, Some("fresh"));
    assert_eq!(label.text(), "fresh");
    assert!(label.get_visible());

    let title = MarqueeLabel::new(hooks::media_shell::TITLE, 180, 32);
    let title_widget = title.widget();
    update_title_label(&title_widget, &title, Some("Track title"));
    assert!(title_widget.get_visible());
    update_title_label(&title_widget, &title, None);
    assert!(!title_widget.get_visible());
}

#[gtk::test]
fn display_config_updates_metadata_visibility_and_shell_classes() {
    let card = card();
    card.source_label.set_visible(true);
    card.position_label.set_visible(false);
    card.title_widget.set_visible(true);

    card.sync_metadata_visibility();

    assert!(card.meta_row.get_visible());
    assert!(card.root.has_css_class(hooks::media_shell::HAS_SOURCE));
    assert!(!card.root.has_css_class(hooks::media_shell::NO_SOURCE));
    assert!(!card.root.has_css_class(hooks::media_shell::HAS_POSITION));
    assert!(card.root.has_css_class(hooks::media_shell::NO_POSITION));
    assert!(card.root.has_css_class(hooks::media_shell::HAS_TITLE));
    assert!(!card.root.has_css_class(hooks::media_shell::NO_TITLE));

    card.source_label.set_visible(false);
    card.title_widget.set_visible(false);
    card.sync_metadata_visibility();
    assert!(!card.meta_row.get_visible());
    assert!(!card.root.has_css_class(hooks::media_shell::HAS_SOURCE));
    assert!(card.root.has_css_class(hooks::media_shell::NO_SOURCE));
    assert!(!card.root.has_css_class(hooks::media_shell::HAS_TITLE));
    assert!(card.root.has_css_class(hooks::media_shell::NO_TITLE));
}

#[gtk::test]
fn apply_display_config_replaces_the_live_media_rules() {
    let card = card();
    let config = MediaConfig {
        show_source: false,
        show_position: false,
        show_title: false,
        ..MediaConfig::default()
    };

    card.apply_display_config(&config);

    let display = card.display.borrow();
    assert!(!display.show_source);
    assert!(!display.show_position);
    assert!(!display.show_title);
}

#[gtk::test]
fn media_state_helpers_cover_each_class_icon_and_capability_transition() {
    let card = card();

    update_artist_classes(&card.root, "Artist");
    assert!(card.root.has_css_class(hooks::media_card::HAS_ARTIST));
    assert!(!card.root.has_css_class(hooks::media_card::EMPTY_ARTIST));
    update_artist_classes(&card.root, "");
    assert!(!card.root.has_css_class(hooks::media_card::HAS_ARTIST));
    assert!(card.root.has_css_class(hooks::media_card::EMPTY_ARTIST));

    update_play_button(&card.play_button, "Playing");
    assert_eq!(
        card.play_button.icon_name().as_deref(),
        Some("media-playback-pause-symbolic")
    );
    update_play_button(&card.play_button, "Paused");
    assert_eq!(
        card.play_button.icon_name().as_deref(),
        Some("media-playback-start-symbolic")
    );

    let mut info = media_info();
    info.can_play = false;
    info.can_pause = false;
    info.can_next = true;
    info.can_prev = false;
    update_control_sensitivity(&card, &info);
    assert!(!card.play_button.is_sensitive());
    assert!(card.next_button.is_sensitive());
    assert!(!card.prev_button.is_sensitive());

    info.can_pause = true;
    info.can_next = false;
    info.can_prev = true;
    update_control_sensitivity(&card, &info);
    assert!(card.play_button.is_sensitive());
    assert!(!card.next_button.is_sensitive());
    assert!(card.prev_button.is_sensitive());
}

#[gtk::test]
fn media_card_classes_follow_playback_art_and_player_count_boundaries() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

    for status in ["Playing", "Paused", "Stopped", "Unknown"] {
        update_playing_class(&root, status);
        assert_eq!(
            root.has_css_class(hooks::shared_state::PLAYING),
            status == "Playing"
        );
        assert_eq!(
            root.has_css_class(hooks::media_card::PLAYING),
            status == "Playing"
        );
        assert_eq!(
            root.has_css_class(hooks::media_card::PAUSED),
            status == "Paused"
        );
        assert_eq!(
            root.has_css_class(hooks::media_card::STOPPED),
            status == "Stopped"
        );
    }

    update_art_classes(&root, true);
    assert!(root.has_css_class(hooks::media_card::HAS_ART));
    assert!(!root.has_css_class(hooks::media_card::NO_ART));
    update_art_classes(&root, false);
    assert!(!root.has_css_class(hooks::media_card::HAS_ART));
    assert!(root.has_css_class(hooks::media_card::NO_ART));

    for total in [0, 1, 2] {
        update_player_count_classes(&root, total);
        assert_eq!(
            root.has_css_class(hooks::media_card::MULTI_PLAYER),
            total > 1
        );
        assert_eq!(
            root.has_css_class(hooks::media_card::SINGLE_PLAYER),
            total <= 1
        );
    }

    set_class_state(&root, "test-state", true);
    set_class_state(&root, "test-state", true);
    assert!(root.has_css_class("test-state"));
    set_class_state(&root, "test-state", false);
    assert!(!root.has_css_class("test-state"));
}
