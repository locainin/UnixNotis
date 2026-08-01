use std::path::{Path, PathBuf};
use std::time::Duration;

use super::super::cache::{NegativeCause, OwnershipLookup};
use super::super::query::{query_package_ownership, PackageProviderCommand};
use super::super::rpm::{query_rpm_owner, query_rpm_ownership_with};
use super::super::{InstallProvenance, PackageProvider};

#[test]
fn rpm_bulk_resolution_maps_each_queried_path() {
    let paths = [
        PathBuf::from("/usr/bin/example-one"),
        PathBuf::from("/usr/bin/example-two"),
        PathBuf::from("/usr/share/applications/example.desktop"),
    ];

    let ownership = query_rpm_ownership_with(&paths, Duration::from_secs(1), &|path, _timeout| {
        let package_id = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("fixture path should have a UTF-8 file name")
            .to_string();
        OwnershipLookup::Known(InstallProvenance::Package {
            provider: PackageProvider::Rpm,
            package_id,
        })
    });

    for path in paths {
        let expected = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("fixture path should have a UTF-8 file name");
        assert_eq!(
            ownership.get(&path),
            Some(&OwnershipLookup::Known(InstallProvenance::Package {
                provider: PackageProvider::Rpm,
                package_id: expected.to_string(),
            })),
            "each RPM query result must remain bound to its requested path"
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

#[test]
fn failed_rpm_process_is_not_reported_as_a_confirmed_unowned_path() {
    let provider = PackageProviderCommand {
        provider: PackageProvider::Rpm,
        executable: PathBuf::from("/bin/false"),
    };

    let result = query_rpm_owner(
        &provider,
        Path::new("/usr/bin/example"),
        Duration::from_secs(1),
    );

    assert_eq!(
        result,
        OwnershipLookup::Negative(NegativeCause::ProviderFailure)
    );
}
