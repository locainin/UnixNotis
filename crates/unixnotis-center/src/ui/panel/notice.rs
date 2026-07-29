//! Configuration and CSS reload notice construction

use gtk::prelude::*;
use unixnotis_core::css::hooks;

pub(super) const RELOAD_NOTICE_TRANSITION_MS: u32 = 160;

pub(in crate::ui) struct ReloadNoticeWidgets {
    pub(in crate::ui) revealer: gtk::Revealer,
    pub(in crate::ui) shell: gtk::Box,
    pub(in crate::ui) label: gtk::Label,
    pub(in crate::ui) close: gtk::Button,
    pub(in crate::ui) actions: gtk::Box,
    pub(in crate::ui) use_stock_button: gtk::Button,
    pub(in crate::ui) open_theme_folder_button: gtk::Button,
}

pub(in crate::ui) fn build_reload_notice() -> ReloadNoticeWidgets {
    // The outer column keeps compatibility choices below the compact status message
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shell.add_css_class(hooks::panel_shell::RELOAD_NOTICE);
    shell.set_hexpand(true);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.add_css_class(hooks::panel_shell::RELOAD_NOTICE_CONTENT);

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

    content.append(&label);
    content.append(&close);
    shell.append(&content);

    // Theme compatibility offers a safe fallback and access to the untouched files
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.add_css_class(hooks::panel_shell::RELOAD_NOTICE_ACTIONS);
    actions.set_homogeneous(true);
    actions.set_visible(false);

    let use_stock_button = notice_action("Use stock theme", "Use bundled UnixNotis styling");
    use_stock_button.add_css_class(hooks::panel_shell::RELOAD_NOTICE_ACTION_PRIMARY);
    let open_theme_folder_button =
        notice_action("Open theme folder", "Open the configured theme folder");
    actions.append(&use_stock_button);
    actions.append(&open_theme_folder_button);
    shell.append(&actions);

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
        close,
        actions,
        use_stock_button,
        open_theme_folder_button,
    }
}

fn notice_action(label: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class(hooks::panel_shell::RELOAD_NOTICE_ACTION);
    button.set_tooltip_text(Some(tooltip));
    button
}

#[cfg(test)]
#[path = "tests/notice.rs"]
mod tests;
