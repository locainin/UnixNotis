use super::*;

#[test]
fn system_desktop_identity_allows_legitimate_signal_reply() {
    let (signal_path, signal_identity) = installed_system_executable();
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.signal.Signal",
            "Signal",
            &signal_path,
            signal_identity,
        )],
        Vec::new(),
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Signal",
            desktop_entry: Some("org.signal.Signal.desktop"),
        },
        &sender(&signal_path, signal_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(
        resolution.attribution.class,
        AttributionClass::SystemAssociated
    );
    assert_eq!(resolution.attribution.display_name, "Signal");
    assert_eq!(
        resolution.attribution.group_key,
        "system-desktop:org.signal.Signal"
    );
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Allow);
    assert!(!resolution.attribution.source_label.contains("unverified"));
}

#[test]
fn dedicated_system_binary_rejects_runtime_added_flags_outside_the_exec_contract() {
    let (signal_path, signal_identity) = installed_system_executable();
    let record = system_record("signal", "Signal", &signal_path, signal_identity)
        .with_launch_literals(&["--", "sgnl://expected"]);
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            // Signal sends an empty app name and adds Electron flags after desktop activation
            reported_name: "",
            desktop_entry: None,
        },
        &sender_with_arguments(
            &signal_path,
            signal_identity,
            &["--password-store=kwallet6", "--ozone-platform=x11", "--"],
        ),
        &index,
        &HashSet::new(),
    );

    assert_ne!(
        resolution.attribution.class,
        AttributionClass::SystemAssociated
    );
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn verified_executable_recovers_from_stale_desktop_hint() {
    let (signal_path, signal_identity) = installed_system_executable();
    let mut stale_user_entry = DesktopRecord::fixture(
        "signal-desktop",
        "Signal",
        "/usr/bin/env",
        identity(90, 900, 0),
        false,
        false,
    );
    // An env wrapper cannot associate the user entry with the dedicated Signal process
    stale_user_entry.association_eligible = false;
    stale_user_entry.system_association = false;
    let system_entry = system_record("signal", "Signal", &signal_path, signal_identity);
    let index =
        DesktopIdentityIndex::from_records(vec![stale_user_entry, system_entry], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Signal",
            // Electron derives this hint from a differently named local desktop file
            desktop_entry: Some("signal-desktop"),
        },
        &sender(&signal_path, signal_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(
        resolution.attribution.class,
        AttributionClass::SystemAssociated
    );
    assert_eq!(resolution.attribution.display_name, "Signal");
    assert_eq!(resolution.attribution.desktop_id, "signal");
}

#[test]
fn empty_claim_cannot_choose_between_apps_sharing_one_dedicated_binary() {
    let (runtime_path, runtime_identity) = installed_system_executable();
    let first = system_record(
        "org.example.First",
        "First App",
        &runtime_path,
        runtime_identity,
    )
    .with_launch_literals(&["--app-id=first"]);
    let second = system_record(
        "org.example.Second",
        "Second App",
        &runtime_path,
        runtime_identity,
    )
    .with_launch_literals(&["--app-id=second"]);
    let index = DesktopIdentityIndex::from_records(vec![first, second], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "",
            desktop_entry: None,
        },
        &sender_with_arguments(&runtime_path, runtime_identity, &["--unmodeled"]),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::Unknown);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn duplicate_desktop_id_prefers_the_protected_record() {
    let (app_path, app_identity) = installed_system_executable();
    let user_record =
        DesktopRecord::fixture("signal", "Signal", &app_path, app_identity, false, false);
    let mut system_record = system_record("signal", "Signal", &app_path, app_identity);
    system_record.badge_icon = "protected-signal".to_string();
    let index = DesktopIdentityIndex::from_records(vec![user_record, system_record], Vec::new());
    let records = index.records_for_executable(app_identity);

    let verified = verified_executable_record(&records, "", &sender(&app_path, app_identity))
        .expect("duplicate desktop id should keep one verified record");

    assert!(verified.0.system_association);
    assert_eq!(verified.0.badge_icon, "protected-signal");
}

#[test]
fn duplicate_protected_desktop_id_keeps_stable_index_order() {
    let (app_path, app_identity) = installed_system_executable();
    let mut first = system_record("signal", "Signal", &app_path, app_identity);
    first.badge_icon = "first-signal".to_string();
    let mut second = system_record("signal", "Signal", &app_path, app_identity);
    second.badge_icon = "second-signal".to_string();
    let index = DesktopIdentityIndex::from_records(vec![first, second], Vec::new());
    let records = index.records_for_executable(app_identity);

    let verified = verified_executable_record(&records, "", &sender(&app_path, app_identity))
        .expect("duplicate protected records should keep one verified record");

    assert_eq!(verified.0.badge_icon, "first-signal");
}

#[test]
fn reopened_system_identity_must_remain_protected_and_executable() {
    let (_, trusted) = installed_system_executable();
    let unprotected = FileIdentity {
        uid: 1_000,
        ..trusted
    };
    let non_executable = FileIdentity {
        mode: 0o100_644,
        ..trusted
    };

    assert!(current_system_identity_matches_sender(trusted, trusted));
    assert!(!current_system_identity_matches_sender(
        unprotected,
        trusted
    ));
    assert!(!current_system_identity_matches_sender(
        non_executable,
        trusted
    ));
}

#[test]
fn stale_cached_system_identity_is_denied_for_explicit_and_no_hint_routes() {
    let (system_path, cached_identity) = installed_system_executable();
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.example.Protected",
            "Protected App",
            &system_path,
            cached_identity,
        )],
        Vec::new(),
    );
    let untrusted_identities = [
        FileIdentity {
            uid: 1_000,
            ..cached_identity
        },
        FileIdentity {
            mode: 0o100_777,
            ..cached_identity
        },
    ];

    for desktop_entry in [Some("org.example.Protected"), None] {
        for sender_identity in untrusted_identities {
            let resolution = resolve_with_evidence(
                AppClaim {
                    reported_name: "Protected App",
                    desktop_entry,
                },
                &sender(&system_path, sender_identity),
                &index,
                &HashSet::new(),
            );

            assert_ne!(
                resolution.attribution.class,
                AttributionClass::SystemAssociated,
                "stale system identity accepted for hint {desktop_entry:?}"
            );
            assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
        }
    }
}

#[test]
fn user_desktop_identity_denies_reply_until_backend_confirmation_exists() {
    let app_identity = identity(6, 60, 1000);
    let index = DesktopIdentityIndex::from_records(
        vec![DesktopRecord::fixture(
            "org.example.LocalApp",
            "Local App",
            "/home/user/bin/local-app",
            app_identity,
            false,
            false,
        )],
        Vec::new(),
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Local App",
            desktop_entry: Some("org.example.LocalApp"),
        },
        &sender("/home/user/bin/local-app", app_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(
        resolution.attribution.class,
        AttributionClass::UserAssociated
    );
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert!(!resolution.attribution.has_warning());
}
