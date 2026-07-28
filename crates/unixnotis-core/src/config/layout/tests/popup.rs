use super::super::PopupConfig;

#[test]
fn popup_defaults_limit_the_visible_stack_to_three_notifications() {
    let popup = PopupConfig::default();

    assert_eq!(popup.max_visible, 3);
}
