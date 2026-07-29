use gtk::prelude::*;

use super::PopupWidthConstraint;

#[gtk::test]
fn unconstrained_vertical_measurement_uses_the_known_surface_width() {
    let label = gtk::Label::new(Some(
        "A wrapping popup body must measure against the fixed layer width",
    ));
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    let constraint = PopupWidthConstraint::new(&label, 240);

    assert_eq!(
        constraint.measure(gtk::Orientation::Vertical, -1),
        label.measure(gtk::Orientation::Vertical, 240)
    );
    assert_eq!(
        constraint.request_mode(),
        gtk::SizeRequestMode::HeightForWidth
    );
    assert_eq!(
        constraint.measure(gtk::Orientation::Horizontal, 1),
        (240, 240, -1, -1)
    );
}

#[gtk::test]
fn updated_surface_width_changes_the_vertical_measurement_contract() {
    let label = gtk::Label::new(Some(
        "A longer wrapping body needs more lines when the popup becomes narrow",
    ));
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    let constraint = PopupWidthConstraint::new(&label, 280);
    let wide = constraint.measure(gtk::Orientation::Vertical, -1);

    constraint.set_width_hint(120);
    let narrow = constraint.measure(gtk::Orientation::Vertical, -1);

    assert!(
        narrow.0 > wide.0,
        "a narrower fixed surface must report a taller minimum"
    );
}
