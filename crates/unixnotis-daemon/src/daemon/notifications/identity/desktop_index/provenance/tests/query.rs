use std::path::PathBuf;

use super::super::cache::{NegativeCause, OwnershipLookup};
use super::super::query::{
    ownership_chunk_len, package_provenance, parse_dpkg_output, parse_pacman_output,
    query_package_ownership, PackageProviderCommand, MAX_COMMAND_ARGUMENT_BYTES, MAX_COMMAND_PATHS,
};
use super::super::{InstallProvenance, PackageProvider};

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
fn short_package_paths_share_one_bounded_provider_query() {
    let paths = [
        PathBuf::from("/usr/bin/example-one"),
        PathBuf::from("/usr/bin/example-two"),
        PathBuf::from("/usr/share/applications/example.desktop"),
    ];

    assert_eq!(ownership_chunk_len(&paths), paths.len());
}

#[test]
fn package_query_chunk_never_exceeds_the_path_count_limit() {
    let paths = (0..=MAX_COMMAND_PATHS)
        .map(|index| PathBuf::from(format!("p{index}")))
        .collect::<Vec<_>>();

    assert_eq!(ownership_chunk_len(&paths), MAX_COMMAND_PATHS);
}

#[test]
fn oversized_first_package_selector_still_advances_exactly_one_path() {
    let paths = [
        PathBuf::from("x".repeat(MAX_COMMAND_ARGUMENT_BYTES.saturating_add(1))),
        PathBuf::from("next"),
    ];

    assert_eq!(ownership_chunk_len(&paths), 1);
}

#[test]
fn package_query_chunk_accepts_the_exact_argument_byte_limit() {
    let first_bytes = MAX_COMMAND_ARGUMENT_BYTES.saturating_sub(3);
    let paths = [PathBuf::from("x".repeat(first_bytes)), PathBuf::from("y")];

    assert_eq!(ownership_chunk_len(&paths), 2);
}

#[test]
fn ownership_query_returns_a_classified_result_for_each_path() {
    let provider = PackageProviderCommand {
        provider: PackageProvider::Pacman,
        executable: PathBuf::from("/bin/echo"),
    };
    let paths = [
        PathBuf::from("/usr/bin/example-one"),
        PathBuf::from("/usr/bin/example-two"),
    ];

    let resolved = query_package_ownership(&provider, &paths);

    assert_eq!(resolved.len(), paths.len());
    for path in paths {
        assert_eq!(
            resolved.get(&path),
            Some(&OwnershipLookup::Negative(NegativeCause::MalformedOutput)),
            "successful but unrecognized provider output must remain a transient failure"
        );
    }
}

#[test]
fn rpm_query_returns_a_classified_result_for_each_path() {
    let provider = PackageProviderCommand {
        provider: PackageProvider::Rpm,
        executable: PathBuf::from("/bin/echo"),
    };
    let paths = [
        PathBuf::from("/usr/bin/example-one"),
        PathBuf::from("/usr/bin/example-two"),
    ];

    let resolved = query_package_ownership(&provider, &paths);

    assert_eq!(resolved.len(), paths.len());
    for path in paths {
        assert!(
            resolved.contains_key(&path),
            "each RPM selector should receive a classified result"
        );
    }
}
