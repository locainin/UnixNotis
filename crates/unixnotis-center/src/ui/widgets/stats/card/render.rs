//! Statistic card value rendering

use std::cell::RefCell;
use std::rc::Rc;

use super::StatItem;

pub(super) fn apply_cached_value(label: &gtk::Label, cache: &Rc<RefCell<Option<String>>>) {
    if let Some(value) = cache.borrow().as_ref() {
        // Stable values avoid an unnecessary GTK property update
        if label.text().as_str() != value {
            label.set_text(value);
        }
    } else if label.text().as_str() != "n/a" {
        // Missing samples share one predictable fallback label
        label.set_text("n/a");
    }
}

impl StatItem {
    pub(super) fn apply_value(&self, value: &str) -> bool {
        if self.last_value.borrow().as_deref() == Some(value) {
            return false;
        }
        // Cache and label change together so fallback reads remain accurate
        self.value_label.set_text(value);
        *self.last_value.borrow_mut() = Some(value.to_string());
        true
    }
}
