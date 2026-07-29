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
fn migration_actions_have_distinct_labels_and_primary_apply_style() {
    let notice = build_reload_notice();

    assert_eq!(notice.preview_button.label().as_deref(), Some("Preview"));
    assert_eq!(notice.apply_button.label().as_deref(), Some("Apply"));
    assert_eq!(notice.keep_button.label().as_deref(), Some("Keep Current"));
    assert!(notice
        .apply_button
        .has_css_class(hooks::panel_shell::RELOAD_NOTICE_ACTION_PRIMARY));
    assert!(!notice
        .preview_button
        .has_css_class(hooks::panel_shell::RELOAD_NOTICE_ACTION_PRIMARY));
    assert!(!notice
        .keep_button
        .has_css_class(hooks::panel_shell::RELOAD_NOTICE_ACTION_PRIMARY));
}
