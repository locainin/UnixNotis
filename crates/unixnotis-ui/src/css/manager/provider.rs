//! GTK CSS provider boundary used by runtime code and tests

use gtk::gdk;
use gtk::CssProvider;

pub(super) trait CssProviderBackend: Clone {
    fn load_css_data(&self, data: &str);
    fn add_to_display(&self, display: &gdk::Display, priority: u32);
}

impl CssProviderBackend for CssProvider {
    fn load_css_data(&self, data: &str) {
        self.load_from_data(data);
    }

    fn add_to_display(&self, display: &gdk::Display, priority: u32) {
        gtk::style_context_add_provider_for_display(display, self, priority);
    }
}
