use super::super::*;

#[test]
fn equivalent_desktop_aliases_use_one_canonical_application_identity() {
    let (app_path, app_identity) = installed_system_executable();
    let mut canonical = system_record("org.example.True", "Example App", &app_path, app_identity);
    canonical.badge_icon = "example-app".to_string();
    let mut alias = system_record(
        "org.example.True.NewWindow",
        "Example App New Window",
        &app_path,
        app_identity,
    );
    alias.badge_icon = "example-app-new-window".to_string();
    canonical.desktop_provenance = package("example-app");
    canonical.executable_provenance = package("example-app");
    alias.desktop_provenance = package("example-app");
    alias.executable_provenance = package("example-app");

    let resolve_alias = |records| {
        let index = DesktopIdentityIndex::from_records(records, Vec::new());
        resolve_with_evidence(
            AppClaim {
                reported_name: "Example App New Window",
                desktop_entry: Some("org.example.True.NewWindow"),
            },
            &sender(&app_path, app_identity),
            &index,
            &HashSet::new(),
        )
    };
    let canonical_first = resolve_alias(vec![canonical.clone(), alias.clone()]);
    let alias_first = resolve_alias(vec![alias, canonical]);

    for resolution in [&canonical_first, &alias_first] {
        assert_eq!(resolution.attribution.status, AttributionStatus::Verified);
        assert_eq!(resolution.attribution.display_name, "Example App");
        assert_eq!(resolution.attribution.badge_icon, "example-app");
        assert_eq!(
            resolution.attribution.group_key,
            "verified:system-app:org.example.True"
        );
    }
    assert_eq!(
        canonical_first.attribution.group_key, alias_first.attribution.group_key,
        "family grouping must not depend on desktop-index insertion order"
    );
}

#[test]
fn fuzzy_name_substrings_do_not_merge_distinct_application_families() {
    let executable = identity(93, 930, 0);
    let mut first = system_record(
        "org.example.Primary",
        "Example",
        "/usr/bin/example",
        executable,
    );
    let mut second = system_record(
        "org.example.Remote",
        "Example Remote",
        "/usr/bin/example",
        executable,
    );
    for record in [&mut first, &mut second] {
        record.desktop_provenance = package("example-suite");
        record.executable_provenance = package("example-suite");
    }
    let index = DesktopIdentityIndex::from_records(vec![first, second], Vec::new());
    let records = index.records_for_executable(executable);

    assert_eq!(records.len(), 2);
    assert!(
        !index.records_share_family(records[0], records[1]),
        "substring-overlapping display names are not application identity evidence"
    );
}

#[test]
fn duplicate_desktop_ids_do_not_make_distinct_families_equal() {
    let first_identity = identity(94, 940, 0);
    let second_identity = identity(95, 950, 0);
    let first = system_record(
        "org.example.Duplicate",
        "First application",
        "/usr/bin/first",
        first_identity,
    );
    let second = system_record(
        "org.example.Duplicate",
        "Second application",
        "/usr/bin/second",
        second_identity,
    );
    let index = DesktopIdentityIndex::from_records(vec![first, second], Vec::new());
    let records = index.records_for_id("org.example.Duplicate");

    assert_eq!(records.len(), 2);
    assert!(
        !index.records_share_family(records[0], records[1]),
        "a reused desktop id cannot replace concrete family identity"
    );
}

#[test]
fn stronger_verified_family_wins_after_weaker_families_are_ambiguous() {
    let first = DesktopRecord::fixture(
        "org.example.UserOne",
        "Shared App",
        "/home/user/one",
        identity(96, 960, 1_000),
        false,
        false,
    );
    let second = DesktopRecord::fixture(
        "org.example.UserTwo",
        "Shared App",
        "/home/user/two",
        identity(97, 970, 1_000),
        false,
        false,
    );
    let system = system_record(
        "org.example.System",
        "Shared App",
        "/usr/bin/system-app",
        identity(98, 980, 0),
    );
    let index = DesktopIdentityIndex::from_records(vec![first, second, system], Vec::new());
    let records = index.records_for_claim("Shared App");
    let results = records
        .iter()
        .map(|record| CandidateVerification {
            record,
            verification: LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable),
        })
        .collect::<Vec<_>>();

    let selected = strongest_verified_result(&results, "Shared App", &index)
        .expect("the strongest unambiguous family should be selected");

    assert_eq!(selected.0.id, "org.example.System");
}

#[test]
fn strongest_verified_family_selects_its_canonical_record() {
    let executable = identity(99, 990, 0);
    let mut canonical = system_record(
        "org.example.Canonical",
        "Example App",
        "/usr/bin/example",
        executable,
    );
    let mut alias = system_record(
        "org.example.Canonical.NewWindow",
        "Example App New Window",
        "/usr/bin/example",
        executable,
    );
    for record in [&mut canonical, &mut alias] {
        record.desktop_provenance = package("example-app");
        record.executable_provenance = package("example-app");
    }
    let index = DesktopIdentityIndex::from_records(vec![alias, canonical], Vec::new());
    let records = index.records_for_executable(executable);
    let results = records
        .iter()
        .map(|record| CandidateVerification {
            record,
            verification: LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable),
        })
        .collect::<Vec<_>>();

    let selected = strongest_verified_result(&results, "Example App New Window", &index)
        .expect("one verified application family should have a canonical selection");

    assert_eq!(selected.0.id, "org.example.Canonical");
}
