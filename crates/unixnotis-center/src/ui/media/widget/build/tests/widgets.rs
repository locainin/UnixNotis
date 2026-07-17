use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use unixnotis_core::{hooks, MediaConfig};

use crate::media::MediaHandle;

use super::super::selection::MediaSelection;
use super::super::shell::MediaShellConfig;
use super::widgets::{
    build_media_widget, build_media_width_boundary, build_navigation_button,
    media_width_boundary_config,
};

#[test]
fn media_width_boundary_config_clips_natural_width_without_scrollbars() {
    // Pure configuration coverage keeps policy regressions cheap to diagnose
    let boundary = media_width_boundary_config(420);

    assert!(
        !boundary.propagate_natural_width,
        "child width must not resize the panel"
    );
    assert_eq!(boundary.content_width, 420);
    assert_eq!(boundary.horizontal_policy, gtk::PolicyType::External);
    assert_eq!(boundary.vertical_policy, gtk::PolicyType::Never);
}

#[gtk::test]
fn media_width_boundary_applies_the_exact_content_budget_and_child() {
    // GTK may insert a viewport, so the real widget tree is checked end to end
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let boundary = build_media_width_boundary(&row, 438);

    assert_eq!(boundary.min_content_width(), 438);
    assert_eq!(boundary.max_content_width(), 438);
    assert!(!boundary.propagates_natural_width());
    assert_eq!(
        boundary.policy(),
        (gtk::PolicyType::External, gtk::PolicyType::Never)
    );
    let viewport = boundary.child().expect("scrolled viewport");
    assert_eq!(
        viewport.first_child().as_ref(),
        Some(row.upcast_ref::<gtk::Widget>())
    );
    assert!(
        boundary.hexpands(),
        "the boundary should consume the available row width"
    );
    assert_eq!(boundary.halign(), gtk::Align::Fill);
    assert_eq!(boundary.overflow(), gtk::Overflow::Hidden);
}

#[gtk::test]
fn navigation_button_exposes_role_hook_and_centered_text() {
    // Theme hooks and child alignment are part of the media CSS contract
    let button = build_navigation_button("<", hooks::media_shell::NAV_PREV);
    let label = button
        .child()
        .and_downcast::<gtk::Label>()
        .expect("navigation label");

    assert!(button.has_css_class(hooks::media_shell::NAV));
    assert!(button.has_css_class(hooks::media_shell::NAV_PREV));
    assert_eq!(button.halign(), gtk::Align::Center);
    assert_eq!(button.valign(), gtk::Align::Center);
    assert_eq!(label.text(), "<");
    assert!((label.xalign() - 0.5).abs() < f32::EPSILON);
    assert!((label.yalign() - 0.5).abs() < f32::EPSILON);
}

#[gtk::test]
fn media_widget_build_keeps_width_requests_in_the_panel_coordinate_space() {
    // A disconnected handle is sufficient because construction sends no media commands
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let handle = MediaHandle::disconnected(runtime.handle().clone());
    let config = MediaConfig::default();
    let shell = MediaShellConfig::from_config(&config);
    let parts = build_media_widget(
        &handle,
        Rc::new(RefCell::new(MediaSelection::default())),
        438,
        &config,
        &shell,
    );

    assert_eq!(parts.root.width_request(), 438);
    assert_eq!(parts.root.height_request(), -1);
    assert_eq!(parts.card.root.width_request(), -1);
    assert_eq!(parts.card.root.height_request(), shell.card_height_px);

    let boundary = parts
        .root
        .first_child()
        .and_downcast::<gtk::ScrolledWindow>()
        .expect("media width boundary");
    let row = boundary
        .child()
        .and_then(|viewport| viewport.first_child())
        .and_downcast::<gtk::Box>()
        .expect("media row");
    assert_eq!(row.width_request(), 438);
    assert_eq!(row.height_request(), -1);
}
