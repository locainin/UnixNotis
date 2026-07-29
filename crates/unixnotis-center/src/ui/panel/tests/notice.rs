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
    assert!(!notice.actions.get_visible());
    assert!(notice.close.get_visible());

    notice.revealer.set_reveal_child(true);
    notice.close.emit_clicked();
    assert!(!notice.revealer.reveals_child());
}

#[gtk::test]
fn compatibility_actions_have_distinct_labels_and_primary_stock_style() {
    let notice = build_reload_notice();

    assert_eq!(
        notice.use_stock_button.label().as_deref(),
        Some("Use stock theme")
    );
    assert_eq!(
        notice.open_theme_folder_button.label().as_deref(),
        Some("Open theme folder")
    );
    assert!(notice
        .use_stock_button
        .has_css_class(hooks::panel_shell::RELOAD_NOTICE_ACTION_PRIMARY));
    assert!(!notice
        .open_theme_folder_button
        .has_css_class(hooks::panel_shell::RELOAD_NOTICE_ACTION_PRIMARY));
}
