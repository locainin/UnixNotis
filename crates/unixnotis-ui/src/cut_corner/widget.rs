//! GTK widget that clips one child to an angled polygon

use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use unixnotis_core::{css::hooks, CutCorners};

use super::geometry::{build_path, contains_point};

mod imp {
    use super::{build_path, contains_point, glib, render_dimension, Cell, CutCorners, RefCell};
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct CutCorner {
        pub(super) child: RefCell<Option<gtk::Widget>>,
        pub(super) corners: Cell<CutCorners>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CutCorner {
        const NAME: &'static str = "UnixNotisCutCorner";
        type Type = super::CutCorner;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            // BinLayout delegates measurement and allocation to the single child
            class.set_layout_manager_type::<gtk::BinLayout>();
            class.set_css_name("unixnotis-cut-corner");
        }
    }

    impl ObjectImpl for CutCorner {
        fn dispose(&self) {
            if let Some(child) = self.child.borrow_mut().take() {
                // Custom child parenting must be undone before the wrapper is finalized
                child.unparent();
            }
        }
    }

    impl WidgetImpl for CutCorner {
        fn contains(&self, x: f64, y: f64) -> bool {
            let widget = self.obj();
            contains_point(
                f64::from(widget.width()),
                f64::from(widget.height()),
                self.corners.get(),
                x,
                y,
            )
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let Some(child) = self.child.borrow().as_ref().cloned() else {
                return;
            };
            if !child.is_visible() {
                return;
            }

            let corners = self.corners.get();
            if !corners.is_active() {
                // The default path avoids creating a render node when no cut is requested
                self.obj().snapshot_child(&child, snapshot);
                return;
            }

            let path = build_path(
                render_dimension(self.obj().width()),
                render_dimension(self.obj().height()),
                corners,
            );
            // GTK records the child until pop and discards pixels outside this polygon
            snapshot.push_fill(&path, gtk::gsk::FillRule::Winding);
            self.obj().snapshot_child(&child, snapshot);
            snapshot.pop();
        }
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "GTK logical dimensions are bounded far below f32's exact integer range"
)]
const fn render_dimension(value: i32) -> f32 {
    value as f32
}

glib::wrapper! {
    /// Single-child container that clips rendering and pointer hits to diagonal corners
    pub struct CutCorner(ObjectSubclass<imp::CutCorner>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl CutCorner {
    /// Build an angled wrapper around one existing widget
    #[must_use]
    pub fn new(child: &impl IsA<gtk::Widget>, corners: CutCorners) -> Self {
        let wrapper: Self = glib::Object::new();
        wrapper.add_css_class(hooks::cut_corner::ROOT);
        wrapper.set_corners(corners);
        wrapper.set_child(Some(child));
        wrapper
    }

    /// Replace the wrapped widget without rebuilding the clipping primitive
    pub fn set_child(&self, child: Option<&impl IsA<gtk::Widget>>) {
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

    /// Return the current wrapped widget
    #[must_use]
    pub fn child(&self) -> Option<gtk::Widget> {
        self.imp().child.borrow().clone()
    }

    /// Apply new corner geometry and invalidate the rendered plate
    pub fn set_corners(&self, corners: CutCorners) {
        if self.imp().corners.replace(corners) != corners {
            self.queue_draw();
        }
    }

    /// Return the active corner geometry
    #[must_use]
    pub fn corners(&self) -> CutCorners {
        self.imp().corners.get()
    }
}
