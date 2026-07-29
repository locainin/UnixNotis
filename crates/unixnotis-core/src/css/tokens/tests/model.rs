#![expect(
    clippy::float_cmp,
    reason = "theme-token resolution returns exact configured and clamped constants"
)]

use super::super::theme_card_style_values;
use crate::ThemeConfig;

#[test]
fn theme_card_style_values_clamp_alpha_and_keep_lengths() {
    let values = theme_card_style_values(&ThemeConfig {
        border_width: 3,
        card_radius: 18,
        card_alpha: 1.5,
        ..ThemeConfig::default()
    });

    assert_eq!(values.border_width_px, 3.0);
    assert_eq!(values.card_radius_px, 18.0);
    assert_eq!(values.card_alpha, 1.0);
}
