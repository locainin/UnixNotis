use gtk::prelude::*;
use unixnotis_core::SliderWidgetConfig;

use super::{build_slider_stack, slider_sublabel};

#[test]
fn slider_sublabel_uses_numeric_fallback_when_unset() {
    assert_eq!(slider_sublabel("", 25.0), "25%");
}

#[test]
fn slider_sublabel_trims_and_clamps_configured_text() {
    let label = slider_sublabel("  abcdefghijklmnopqrstuvwxyz0123456789  ", 0.0);

    assert_eq!(label, "abcdefghijklmnopqrstuvwxyz012345");
}

#[test]
fn slider_sublabel_clamps_by_chars_not_bytes() {
    let label = slider_sublabel("å".repeat(40).as_str(), 0.0);

    assert_eq!(label.chars().count(), 32);
}

#[test]
fn slider_sublabel_preserves_meaningful_whitespace_inside_label() {
    assert_eq!(slider_sublabel("  low power  ", 0.0), "low power");
}

#[gtk::test]
fn slider_stack_adds_configured_segments_and_sublabels() {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    let config = SliderWidgetConfig {
        segments: 3,
        show_sublabels: true,
        ..SliderWidgetConfig::default()
    };

    let stack = build_slider_stack(&scale, &config);
    let segment_row = scale
        .next_sibling()
        .expect("segment row should follow the scale")
        .downcast::<gtk::Box>()
        .expect("segment row should be a box");
    let sublabel_row = segment_row
        .next_sibling()
        .expect("sublabel row should follow segments")
        .downcast::<gtk::Box>()
        .expect("sublabel row should be a box");

    assert_eq!(stack.observe_children().n_items(), 3);
    assert_eq!(segment_row.observe_children().n_items(), 3);
    assert_eq!(sublabel_row.observe_children().n_items(), 3);
}

#[gtk::test]
fn slider_stack_omits_optional_rows_when_disabled() {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    let config = SliderWidgetConfig {
        segments: 0,
        show_sublabels: false,
        ..SliderWidgetConfig::default()
    };

    let stack = build_slider_stack(&scale, &config);

    assert_eq!(stack.observe_children().n_items(), 1);
    assert!(scale.next_sibling().is_none());
}
