use super::super::{InstallProvenance, PackageProvider};

#[test]
fn matching_package_sources_establish_one_installation_owner() {
    let desktop = InstallProvenance::Package {
        provider: PackageProvider::Pacman,
        package_id: "example-app".to_string(),
    };
    let executable = desktop.clone();

    assert!(desktop.same_application_source(&executable));
    assert!(
        !desktop.same_application_source(&InstallProvenance::Package {
            provider: PackageProvider::Pacman,
            package_id: "shared-runtime".to_string(),
        })
    );
    assert!(!desktop.same_application_source(&InstallProvenance::Unknown));
}
#[test]
fn bundle_and_portal_provenance_require_exact_domain_identity() {
    let bundle = InstallProvenance::ImmutableBundle {
        bundle_id: "org.example.App".to_string(),
    };
    let same_bundle = bundle.clone();
    let other_bundle = InstallProvenance::ImmutableBundle {
        bundle_id: "org.example.Other".to_string(),
    };
    let portal = InstallProvenance::Portal {
        app_id: "org.example.App".to_string(),
    };
    let same_portal = portal.clone();
    let other_portal = InstallProvenance::Portal {
        app_id: "org.example.Other".to_string(),
    };

    assert!(bundle.same_application_source(&same_bundle));
    assert!(!bundle.same_application_source(&other_bundle));
    assert!(!bundle.same_application_source(&portal));
    assert!(portal.same_application_source(&same_portal));
    assert!(!portal.same_application_source(&other_portal));
}
