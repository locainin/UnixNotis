//! Configuration and CSS reload notice construction

use gtk::prelude::*;
use unixnotis_core::css::hooks;

pub(super) const RELOAD_NOTICE_TRANSITION_MS: u32 = 160;

pub(in crate::ui) struct ReloadNoticeWidgets {
    pub(in crate::ui) revealer: gtk::Revealer,
    pub(in crate::ui) shell: gtk::Box,
    pub(in crate::ui) label: gtk::Label,
}

pub(super) fn build_reload_notice() -> ReloadNoticeWidgets {
    // Horizontal layout keeps the message and dismissal action on one row
    let shell = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    shell.add_css_class(hooks::panel_shell::RELOAD_NOTICE);
    shell.set_hexpand(true);

    // Wrapping prevents long parser errors from changing panel width
    let label = gtk::Label::new(None);
    label.add_css_class(hooks::panel_shell::RELOAD_NOTICE_TEXT);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    label.set_hexpand(true);

    // Dismissal hides only the current fingerprinted failure
    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.add_css_class(hooks::panel_shell::RELOAD_NOTICE_CLOSE);
    close.set_tooltip_text(Some("Dismiss reload notice"));
    close.set_valign(gtk::Align::Start);

    shell.append(&label);
    shell.append(&close);

    // A short vertical transition keeps the header position stable
    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    revealer.set_transition_duration(RELOAD_NOTICE_TRANSITION_MS);
    revealer.set_reveal_child(false);
    revealer.set_child(Some(&shell));

    let hidden_revealer = revealer.clone();
    close.connect_clicked(move |_| hidden_revealer.set_reveal_child(false));

    ReloadNoticeWidgets {
        revealer,
        shell,
        label,
    }
}

#[cfg(test)]
#[path = "tests/notice.rs"]
mod tests;
