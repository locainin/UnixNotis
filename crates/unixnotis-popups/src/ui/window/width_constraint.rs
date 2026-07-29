//! Fixed-width measurement bridge for height-for-width popup content

use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::{glib, Cell, RefCell};
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct PopupWidthConstraint {
        pub(super) child: RefCell<Option<gtk::Widget>>,
        pub(super) width_hint: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PopupWidthConstraint {
        const NAME: &'static str = "UnixNotisPopupWidthConstraint";
        type Type = super::PopupWidthConstraint;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_css_name("unixnotis-popup-width-constraint");
        }
    }

    impl ObjectImpl for PopupWidthConstraint {
        fn dispose(&self) {
            if let Some(child) = self.child.borrow_mut().take() {
                // Custom parenting must be released before GTK finalizes the wrapper
                child.unparent();
            }
        }
    }

    impl WidgetImpl for PopupWidthConstraint {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let width_hint = self.width_hint.get().max(1);
            if orientation == gtk::Orientation::Horizontal {
                // The layer surface owns width, so content never expands or contracts it
                return (width_hint, width_hint, -1, -1);
            }

            let Some(child) = self.child.borrow().as_ref().cloned() else {
                return (0, 0, -1, -1);
            };

            // GTK asks for an unconstrained vertical minimum before layer-shell
            // supplies the fixed surface width. Reuse that known width so wrapping
            // text reports the same height in both passes
            let child_for_size = if for_size < 0 { width_hint } else { for_size };
            child.measure(orientation, child_for_size)
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(child) = self.child.borrow().as_ref().cloned() else {
                return;
            };
            // The wrapper has no visual box of its own, so the child receives all space
            child.allocate(width, height, baseline, None);
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let Some(child) = self.child.borrow().as_ref().cloned() else {
                return;
            };
            if child.is_visible() {
                // Custom parenting requires explicit snapshot delegation
                self.obj().snapshot_child(&child, snapshot);
            }
        }
    }
}

glib::wrapper! {
    pub struct PopupWidthConstraint(ObjectSubclass<imp::PopupWidthConstraint>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PopupWidthConstraint {
    pub(super) fn new(child: &impl IsA<gtk::Widget>, width_hint: i32) -> Self {
        let constraint: Self = glib::Object::new();
        constraint.set_child(Some(child));
        constraint.set_width_hint(width_hint);
        constraint
    }

    pub(super) fn set_width_hint(&self, width_hint: i32) {
        let width_hint = width_hint.max(1);
        if self.imp().width_hint.replace(width_hint) != width_hint {
            // A config or monitor change can alter line wrapping and total height
            self.queue_resize();
        }
    }

    fn set_child(&self, child: Option<&impl IsA<gtk::Widget>>) {
        let imp = self.imp();
        let next = child.map(|child| child.clone().upcast::<gtk::Widget>());
        if imp.child.borrow().as_ref() == next.as_ref() {
            return;
        }
        if let Some(current) = imp.child.borrow_mut().take() {
            current.unparent();
        }
        if let Some(next) = next {
            next.set_parent(self);
            imp.child.replace(Some(next));
        }
        self.queue_resize();
    }
}

#[cfg(test)]
#[path = "tests/width_constraint.rs"]
mod tests;
