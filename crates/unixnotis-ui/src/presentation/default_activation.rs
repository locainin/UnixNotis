//! Shared whole-card default activation for popup and panel rows

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::NotificationKey;

pub const INTERACTIVE_CLASS: &str = "unixnotis-popup-interactive";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultActionTarget {
    pub notification: NotificationKey,
    pub action_key: String,
}

#[derive(Clone)]
pub struct DefaultActionBinding {
    target: Rc<RefCell<Option<DefaultActionTarget>>>,
    root: gtk::glib::WeakRef<gtk::Widget>,
}

impl DefaultActionBinding {
    pub fn set_target(&self, target: Option<DefaultActionTarget>) {
        let enabled = target.is_some();
        *self.target.borrow_mut() = target;

        let Some(root) = self.root.upgrade() else {
            return;
        };

        // Recycled rows are only keyboard controls while an active generation is bound
        root.set_focusable(enabled);
        root.set_accessible_role(if enabled {
            gtk::AccessibleRole::Button
        } else {
            gtk::AccessibleRole::Generic
        });
        root.update_property(&[gtk::accessible::Property::Label(if enabled {
            "Open notification"
        } else {
            ""
        })]);
        if enabled {
            root.add_css_class("unixnotis-popup-default-action");
        } else {
            root.remove_css_class("unixnotis-popup-default-action");
        }
    }
}

pub fn mark_interactive<W: IsA<gtk::Widget>>(widget: &W) {
    // Composite controls use the marker even when their leaf widget changes
    widget.add_css_class(INTERACTIVE_CLASS);
}

pub fn connect_default_activation<W, F>(widget: &W, dispatch: F) -> DefaultActionBinding
where
    W: IsA<gtk::Widget>,
    F: Fn(NotificationKey, String) + 'static,
{
    let root = widget.clone().upcast::<gtk::Widget>();

    let target: Rc<RefCell<Option<DefaultActionTarget>>> = Rc::new(RefCell::new(None));
    let dispatch = Rc::new(dispatch);

    let gesture = gtk::GestureClick::new();
    gesture.set_button(1);
    let click_root = root.downgrade();
    let click_target = Rc::clone(&target);
    let click_dispatch = Rc::clone(&dispatch);
    gesture.connect_released(move |_, _, x, y| {
        let Some(root) = click_root.upgrade() else {
            return;
        };
        let Some(current) = click_target.borrow().clone() else {
            return;
        };
        if picked_widget_blocks_default_action(&root, root.pick(x, y, gtk::PickFlags::DEFAULT)) {
            return;
        }
        click_dispatch(current.notification, current.action_key);
    });
    root.add_controller(gesture);

    let key_controller = gtk::EventControllerKey::new();
    let key_root = root.downgrade();
    let key_target = Rc::clone(&target);
    let key_dispatch = Rc::clone(&dispatch);
    key_controller.connect_key_pressed(move |_, key, _, _| {
        let Some(root) = key_root.upgrade() else {
            return gtk::glib::Propagation::Proceed;
        };
        let current = key_target.borrow().clone();
        if keyboard_activation_is_ready(root.has_focus(), key, current.is_some()) {
            if let Some(current) = current {
                key_dispatch(current.notification, current.action_key);
                return gtk::glib::Propagation::Stop;
            }
        }
        gtk::glib::Propagation::Proceed
    });
    root.add_controller(key_controller);

    let binding = DefaultActionBinding {
        target,
        root: root.downgrade(),
    };
    binding.set_target(None);
    binding
}

#[must_use]
pub fn picked_widget_blocks_default_action(
    root: &gtk::Widget,
    mut picked: Option<gtk::Widget>,
) -> bool {
    while let Some(current) = picked {
        if current == *root {
            return false;
        }
        if current.has_css_class(INTERACTIVE_CLASS)
            || current.is_focusable()
            || current.is::<gtk::Button>()
            || current.is::<gtk::MenuButton>()
            || current.is::<gtk::Entry>()
        {
            return true;
        }
        picked = current.parent();
    }
    false
}

#[must_use]
pub const fn is_default_activation_key(key: gtk::gdk::Key) -> bool {
    matches!(
        key,
        gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter | gtk::gdk::Key::space
    )
}

pub(super) const fn keyboard_activation_is_ready(
    root_has_focus: bool,
    key: gtk::gdk::Key,
    has_target: bool,
) -> bool {
    root_has_focus && has_target && is_default_activation_key(key)
}

#[cfg(test)]
#[path = "tests/default_activation.rs"]
mod tests;
