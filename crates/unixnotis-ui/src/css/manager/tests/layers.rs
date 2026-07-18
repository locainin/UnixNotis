use super::{CssProviderLayer, CssProviderRegistration};

#[test]
fn internal_structure_layer_remains_distinct_from_user_theme_layers() {
    let registration = CssProviderRegistration {
        layer: CssProviderLayer::InternalStructure,
        priority: 1,
    };

    assert_eq!(registration.layer, CssProviderLayer::InternalStructure);
    assert_ne!(registration.layer, CssProviderLayer::Base);
    assert_ne!(registration.layer, CssProviderLayer::Panel);
    assert_eq!(registration.priority, 1);
}
