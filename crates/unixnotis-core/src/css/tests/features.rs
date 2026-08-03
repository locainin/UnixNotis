use super::{
    gtk_css_features_for_version, gtk_css_features_from_version_string, GTK_MIN_VERSION_LABEL,
};

#[test]
fn gtk_css_features_gate_common_apis_at_gtk_418() {
    assert!(!gtk_css_features_for_version(4, 17).custom_properties);
    assert!(gtk_css_features_for_version(4, 18).custom_properties);
    assert!(gtk_css_features_for_version(5, 0).custom_properties);
}

#[test]
fn gtk_css_features_can_parse_pkg_config_versions() {
    assert!(
        !gtk_css_features_from_version_string("4.17.9")
            .expect("version")
            .custom_properties
    );
    assert!(
        gtk_css_features_from_version_string("4.18.3")
            .expect("version")
            .custom_properties
    );
    assert!(
        gtk_css_features_from_version_string("4.18.0-2")
            .expect("version")
            .custom_properties
    );
}

#[test]
fn custom_properties_requirement_label_stays_stable() {
    assert_eq!(GTK_MIN_VERSION_LABEL, "GTK 4.18+");
}
