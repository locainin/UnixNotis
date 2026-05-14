use unixnotis_core::{PanelClearButtonPlacement, PanelConfig};

use super::notification_header_row_visible;

#[test]
fn notification_header_row_stays_visible_for_header_clear_button() {
    let mut config = PanelConfig {
        notification_section_visible: false,
        clear_button_placement: PanelClearButtonPlacement::NotificationHeader,
        ..PanelConfig::default()
    };
    assert!(notification_header_row_visible(&config));

    config.clear_button_placement = PanelClearButtonPlacement::ActionRow;
    assert!(!notification_header_row_visible(&config));
}

#[test]
fn notification_header_row_uses_section_label_when_section_is_visible() {
    let config = PanelConfig {
        notification_section_visible: true,
        recent_notifications_label: "Recent".to_string(),
        clear_button_placement: PanelClearButtonPlacement::Hidden,
        ..PanelConfig::default()
    };

    assert!(notification_header_row_visible(&config));
}
