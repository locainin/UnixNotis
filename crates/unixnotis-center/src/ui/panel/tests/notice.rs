use gtk::prelude::*;
use unixnotis_core::css::hooks;

use super::build_reload_notice;

#[gtk::test]
fn reload_notice_starts_hidden_and_dismiss_button_hides_it() {
    let notice = build_reload_notice();

    assert!(!notice.revealer.reveals_child());
    assert!(notice
        .shell
        .has_css_class(hooks::panel_shell::RELOAD_NOTICE));
    assert!(notice
        .label
        .has_css_class(hooks::panel_shell::RELOAD_NOTICE_TEXT));
    assert!(notice.close.get_visible());

    notice.revealer.set_reveal_child(true);
    notice.close.emit_clicked();
    assert!(!notice.revealer.reveals_child());
}
