use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::{
    ownership_chunk_len, package_provenance, parse_dpkg_output, parse_pacman_output,
    query_package_ownership, query_rpm_owner, query_rpm_ownership_with, run_package_query,
    run_package_query_with_timeout, CachedProvenance, InstallProvenance, NegativeCause,
    OwnershipLookup, PackageOwnershipCache, PackageProvider, PackageProviderCommand,
    PackageQueryFailure, MAX_COMMAND_ARGUMENT_BYTES, MAX_COMMAND_PATHS, NOT_OWNED_NEGATIVE_TTL,
    TRANSIENT_NEGATIVE_TTL,
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

    assert!(
        matches!(output, Err(PackageQueryFailure::Timeout)),
        "a stalled provider should report its deadline"
    );
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
        run_package_query_with_timeout(&mut command, 4, Duration::from_secs(1)).is_err(),
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

#[test]
fn package_query_returns_when_descendant_holds_stdout_open() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "(sleep 2) & exit 0"]);
    let started = Instant::now();

    let output = run_package_query_with_timeout(&mut command, 1024, Duration::from_millis(100));

    assert!(
        matches!(output, Err(PackageQueryFailure::PipeDrainTimeout)),
        "an inherited output pipe should report a bounded drain timeout"
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "an inherited output pipe must not block desktop-index construction"
    );
}

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
fn transient_ownership_failures_expire_before_confirmed_not_owned_entries() {
    let now = Instant::now();
    let transient =
        CachedProvenance::from_lookup(OwnershipLookup::Negative(NegativeCause::Timeout), now);
    let not_owned =
        CachedProvenance::from_lookup(OwnershipLookup::Negative(NegativeCause::NotOwned), now);

    assert!(!transient.needs_refresh(now));
    assert!(transient.needs_refresh(now + TRANSIENT_NEGATIVE_TTL));
    assert!(!not_owned.needs_refresh(now + TRANSIENT_NEGATIVE_TTL));
    assert!(not_owned.needs_refresh(now + NOT_OWNED_NEGATIVE_TTL));
    assert_eq!(transient.provenance(), InstallProvenance::Unknown);
}

#[test]
fn cached_known_provenance_is_returned_for_every_requested_path() {
    let path = PathBuf::from("/usr/bin/example-cache-entry");
    let expected = InstallProvenance::Package {
        provider: PackageProvider::Pacman,
        package_id: "example-cache-entry".to_string(),
    };
    let cache = PackageOwnershipCache::default();
    cache
        .entries
        .lock()
        .expect("package cache should be writable")
        .insert(path.clone(), CachedProvenance::Known(expected.clone()));

    let resolved = cache.resolve_many([path.clone()]);

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved.get(&path), Some(&expected));
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

#[test]
fn timed_out_package_provider_is_terminated_before_returning() {
    let serial = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should follow the Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-package-timeout-{}-{}",
        std::process::id(),
        serial
    ));
    fs::create_dir_all(&root).expect("package timeout test root should be created");
    let pid_file = root.join("provider.pid");
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "printf '%s' \"$$\" > \"$1\"; exec sleep 2",
            "unixnotis-package-timeout",
        ])
        .arg(&pid_file);

    let result = run_package_query_with_timeout(&mut command, 1024, Duration::from_millis(100));
    assert!(matches!(result, Err(PackageQueryFailure::Timeout)));
    let pid = fs::read_to_string(&pid_file).expect("provider should publish its process id");
    let process_path = Path::new("/proc").join(pid.trim());
    let reap_deadline = Instant::now() + Duration::from_millis(250);
    while process_path.exists() && Instant::now() < reap_deadline {
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(
        !process_path.exists(),
        "a timed-out provider must not continue after the ownership query returns"
    );
    fs::remove_dir_all(root).expect("package timeout test root should be removable");
}
