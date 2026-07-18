use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use super::{build_refresh_state_from_weak, SliderRefreshMeta};
use crate::ui::widgets::utils::command_slider::refresh::SliderRefreshGate;
use crate::ui::widgets::utils::RefreshBackoff;

#[gtk::test]
fn weak_widget_state_builds_while_every_widget_is_alive() {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    let label = gtk::Label::new(Some("15%"));
    let icon = gtk::Image::from_icon_name("test-normal");
    let meta = refresh_meta();

    let state = build_refresh_state_from_weak(
        &scale.downgrade(),
        &label.downgrade(),
        &icon.downgrade(),
        &meta,
    )
    .expect("live widgets should produce refresh state");

    assert_eq!(state.label.text(), "15%");
    assert_eq!(state.icon_name, "test-normal");
    assert_eq!(state.icon_muted.as_deref(), Some("test-muted"));
    assert!(Rc::ptr_eq(&state.updating, &meta.updating));
    assert!(Rc::ptr_eq(&state.refresh_gen, &meta.refresh_gen));
    assert!(Rc::ptr_eq(&state.backoff, &meta.backoff));
}

#[gtk::test]
fn weak_widget_state_stops_after_any_widget_is_dropped() {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    let label = gtk::Label::new(None);
    let icon = gtk::Image::new();
    let scale_weak = scale.downgrade();
    let label_weak = label.downgrade();
    let icon_weak = icon.downgrade();
    let meta = refresh_meta();
    drop(label);

    assert!(build_refresh_state_from_weak(&scale_weak, &label_weak, &icon_weak, &meta).is_none());
}

fn refresh_meta() -> SliderRefreshMeta {
    SliderRefreshMeta {
        updating: Rc::new(Cell::new(false)),
        refresh_gen: Rc::new(Cell::new(7)),
        icon_name: "test-normal".to_string(),
        icon_muted: Some("test-muted".to_string()),
        gate: SliderRefreshGate::new(),
        backoff: Rc::new(RefCell::new(RefreshBackoff::default())),
    }
}
