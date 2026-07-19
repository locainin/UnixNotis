use super::{
    popup_action_is_visible, popup_header_spacer_expands, widget_type_blocks_default_action,
};
use gtk::glib::prelude::StaticType;
use unixnotis_core::Action;

#[test]
fn popup_header_spacer_expands_to_hold_close_alignment() {
    // The spacer owns unused header width so the close button stays aligned
    assert!(popup_header_spacer_expands());
}

#[gtk::test]
fn default_card_action_is_blocked_for_button_widgets() {
    // Button clicks must remain owned by the button action
    assert!(widget_type_blocks_default_action(gtk::Button::static_type()));
}

#[gtk::test]
fn default_card_action_is_allowed_for_plain_content_widgets() {
    // Plain card content may use the notification default action
    assert!(!widget_type_blocks_default_action(gtk::Label::static_type()));
}

#[test]
fn popup_actions_hide_inline_reply_but_keep_regular_buttons() {
    let reply = Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    };
    let open = Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    };

    assert!(!popup_action_is_visible(&reply));
    assert!(popup_action_is_visible(&open));
}
