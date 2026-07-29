use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use super::{
    package_provenance, parse_dpkg_output, parse_pacman_output, run_package_query,
    run_package_query_with_timeout, InstallProvenance, PackageProvider,
};

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

#[test]
fn pacman_output_is_mapped_to_the_exact_queried_path() {
    let desktop = PathBuf::from("/usr/share/applications/example.desktop");
    let executable = PathBuf::from("/usr/bin/example");
    let output = b"/usr/bin/example is owned by example-app 2.0-1\n\
/usr/share/applications/example.desktop is owned by example-app 2.0-1\n";

    let ownership = parse_pacman_output(
        output,
        &[desktop.clone(), executable.clone()],
        PackageProvider::Pacman,
    );

    for path in [desktop, executable] {
        assert_eq!(
            ownership.get(&path),
            Some(&InstallProvenance::Package {
                provider: PackageProvider::Pacman,
                package_id: "example-app".to_string(),
            }),
            "the exact queried file should retain its package owner"
        );
    }
}

#[test]
fn dpkg_output_keeps_architecture_qualified_package_identity() {
    let executable = PathBuf::from("/usr/bin/example");
    let ownership = parse_dpkg_output(
        b"example-app:amd64: /usr/bin/example\n",
        std::slice::from_ref(&executable),
        PackageProvider::Dpkg,
    );

    assert_eq!(
        ownership.get(&executable),
        Some(&InstallProvenance::Package {
            provider: PackageProvider::Dpkg,
            package_id: "example-app:amd64".to_string(),
        })
    );
}

#[test]
fn malformed_package_identity_is_rejected() {
    assert!(package_provenance(PackageProvider::Pacman, b"").is_none());
    assert!(package_provenance(PackageProvider::Pacman, b"bad package").is_none());
    assert!(package_provenance(PackageProvider::Pacman, b"bad\npackage").is_none());
}

#[test]
fn package_query_deadline_stops_a_stalled_provider() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 2"]);
    let started = Instant::now();

    let output = run_package_query_with_timeout(&mut command, 1024, Duration::from_millis(20));

    assert!(output.is_none());
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "the package provider deadline should stop a stalled process promptly"
    );
}

#[test]
fn package_query_rejects_output_beyond_the_declared_limit() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 12345"]);

    assert!(
        run_package_query_with_timeout(&mut command, 4, Duration::from_secs(1)).is_none(),
        "oversized provider output must fail closed"
    );
}

#[test]
fn package_query_accepts_successful_output_at_the_exact_limit() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 1234"]);

    let output = run_package_query(&mut command, 4)
        .expect("successful provider output at the exact limit should be retained");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"1234");
}
