//! Shared label helpers for info cards

pub(super) fn apply_cached_value(
    label: &gtk::Label,
    cache: &std::rc::Rc<std::cell::RefCell<Option<String>>>,
) {
    if let Some(value) = cache.borrow().as_ref() {
        if label.text().as_str() != value {
            // Reuse the cached text so transient command failures do not blank the card
            label.set_text(value);
        }
    } else if label.text().as_str() != "n/a" {
        // Default placeholder keeps card layout stable when no prior value exists
        label.set_text("n/a");
    }
}
