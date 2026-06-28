use unixnotis_core::gtk_css_features_from_version_string;

#[test]
fn gtk_css_feature_parser_handles_major_and_minor_checks() {
    // GTK 4.16 is the first modern CSS feature level needed by the shipped theme path
    assert!(
        gtk_css_features_from_version_string("4.16.2")
            .expect("version")
            .custom_properties
    );
    assert!(
        gtk_css_features_from_version_string("4.18")
            .expect("version")
            .custom_properties
    );

    // Older GTK4 builds still work with legacy CSS but should not claim var() support
    assert!(
        !gtk_css_features_from_version_string("4.14.9")
            .expect("version")
            .custom_properties
    );

    // Future major versions should not regress feature detection
    assert!(
        gtk_css_features_from_version_string("5.0.0")
            .expect("version")
            .custom_properties
    );
}
