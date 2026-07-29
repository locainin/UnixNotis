use super::*;

#[test]
fn python_desktop_entry_cannot_trust_an_unrelated_python_process() {
    let python_identity = identity(20, 200, 0);
    let mut record = system_record(
        "org.example.PasswordManager",
        "Example Password Manager",
        "/usr/bin/python3",
        python_identity,
    );
    record.association_eligible = false;
    record.system_association = false;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example Password Manager",
            desktop_entry: Some("org.example.PasswordManager"),
        },
        &sender("/usr/bin/python3", python_identity),
        &index,
        &HashSet::new(),
    );

    assert_ne!(resolution.attribution.status, AttributionStatus::Verified);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn unlisted_runtimes_cannot_associate_a_different_application_payload() {
    for (serial, executable, expected, actual) in [
        (
            1_u64,
            "/usr/bin/pypy3",
            "/usr/share/app/main.py",
            "/tmp/fake.py",
        ),
        (2, "/usr/bin/gjs", "/usr/share/app/main.js", "/tmp/fake.js"),
        (
            3,
            "/usr/bin/dotnet",
            "/usr/share/app/Example.dll",
            "/tmp/Fake.dll",
        ),
    ] {
        let runtime_identity = identity(50, 500 + serial, 0);
        let record = system_record(
            "org.example.RuntimeApp",
            "Runtime App",
            executable,
            runtime_identity,
        )
        .with_launch_literals(&[expected]);
        let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

        let resolution = resolve_with_evidence(
            AppClaim {
                reported_name: "Runtime App",
                desktop_entry: Some("org.example.RuntimeApp"),
            },
            &sender_with_arguments(executable, runtime_identity, &[actual]),
            &index,
            &HashSet::new(),
        );

        assert_ne!(
            resolution.attribution.status,
            AttributionStatus::Verified,
            "{executable} accepted a different application payload"
        );
        assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    }
}

#[test]
fn java_cannot_associate_a_different_jar() {
    let java_identity = identity(51, 510, 0);
    let record = system_record(
        "org.example.JavaApp",
        "Java App",
        "/usr/bin/java",
        java_identity,
    )
    .with_launch_literals(&["-jar", "/usr/share/java/example.jar"]);
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Java App",
            desktop_entry: Some("org.example.JavaApp"),
        },
        &sender_with_arguments("/usr/bin/java", java_identity, &["-jar", "/tmp/fake.jar"]),
        &index,
        &HashSet::new(),
    );

    assert_ne!(resolution.attribution.status, AttributionStatus::Verified);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn matching_fixed_system_application_argument_allows_association() {
    let (runtime_path, runtime_identity) = installed_system_executable();
    let (payload_path, payload_identity) = installed_system_executable();
    let record = system_record(
        "org.example.ScriptApp",
        "Script App",
        &runtime_path,
        runtime_identity,
    )
    .with_launch_literals(&[&payload_path])
    .with_protected_launch_file(&payload_path, payload_identity);
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Script App",
            desktop_entry: Some("org.example.ScriptApp"),
        },
        &sender_with_arguments(&runtime_path, runtime_identity, &[&payload_path]),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Verified);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Allow);
}

#[test]
fn arbitrary_executable_name_still_requires_its_fixed_application_payload() {
    let (launcher_path, launcher_identity) = installed_system_executable();
    let record = system_record(
        "org.example.CustomRuntime",
        "Custom Runtime App",
        &launcher_path,
        launcher_identity,
    )
    .with_launch_literals(&["/usr/share/custom-runtime/application.bin"]);
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Custom Runtime App",
            desktop_entry: Some("org.example.CustomRuntime"),
        },
        &sender_with_arguments(
            &launcher_path,
            launcher_identity,
            &["/tmp/attacker-controlled.bin"],
        ),
        &index,
        &HashSet::new(),
    );

    assert_ne!(resolution.attribution.status, AttributionStatus::Verified);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn dedicated_executable_remains_verified_when_command_line_is_unavailable() {
    let (launcher_path, launcher_identity) = installed_system_executable();
    let record = system_record(
        "org.example.True",
        "Command Line App",
        &launcher_path,
        launcher_identity,
    );
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());
    let mut missing_command_line = sender(&launcher_path, launcher_identity);
    missing_command_line.command_line = CommandLineEvidence::default();

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Command Line App",
            desktop_entry: Some("org.example.True"),
        },
        &missing_command_line,
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Verified);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Allow);
}

#[test]
fn no_hint_shared_runtimes_with_wrong_payloads_are_denied() {
    for (serial, executable, expected, actual) in [
        (
            1_u64,
            "/usr/bin/python3",
            "/usr/share/password-manager/main.py",
            "/tmp/fake.py",
        ),
        (
            2,
            "/usr/bin/pypy3",
            "/usr/share/password-manager/main.py",
            "/tmp/fake.py",
        ),
        (
            3,
            "/usr/bin/gjs",
            "/usr/share/password-manager/main.js",
            "/tmp/fake.js",
        ),
        (
            4,
            "/usr/bin/dotnet",
            "/usr/share/password-manager/PasswordManager.dll",
            "/tmp/Fake.dll",
        ),
        (
            5,
            "/usr/bin/java",
            "/usr/share/password-manager/password-manager.jar",
            "/tmp/fake.jar",
        ),
    ] {
        let runtime_identity = identity(60, 600 + serial, 0);
        let fixed_arguments = if executable == "/usr/bin/java" {
            vec!["-jar", expected]
        } else {
            vec![expected]
        };
        let sender_arguments = if executable == "/usr/bin/java" {
            vec!["-jar", actual]
        } else {
            vec![actual]
        };
        let record = system_record(
            "org.example.PasswordManager",
            "Example Password Manager",
            executable,
            runtime_identity,
        )
        .with_launch_literals(&fixed_arguments);
        let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

        let resolution = resolve_with_evidence(
            AppClaim {
                reported_name: "Example Password Manager",
                desktop_entry: None,
            },
            &sender_with_arguments(executable, runtime_identity, &sender_arguments),
            &index,
            &HashSet::new(),
        );

        assert_ne!(
            resolution.attribution.status,
            AttributionStatus::Verified,
            "{executable} accepted a different no-hint application payload"
        );
        assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    }
}

#[test]
fn no_hint_shared_runtime_with_matching_protected_payload_is_allowed() {
    let (runtime_path, runtime_identity) = installed_system_executable();
    let (payload_path, payload_identity) = installed_system_executable();
    let record = system_record(
        "org.example.PasswordManager",
        "Example Password Manager",
        &runtime_path,
        runtime_identity,
    )
    .with_launch_literals(&[&payload_path])
    .with_protected_launch_file(&payload_path, payload_identity);
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example Password Manager",
            desktop_entry: None,
        },
        &sender_with_arguments(&runtime_path, runtime_identity, &[&payload_path]),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Verified);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Allow);
}

#[test]
fn no_hint_wrong_payload_with_unrelated_claim_remains_unknown() {
    let runtime_identity = identity(62, 620, 0);
    let record = system_record(
        "org.example.PasswordManager",
        "Example Password Manager",
        "/usr/bin/python3",
        runtime_identity,
    )
    .with_launch_literals(&["/usr/share/password-manager/main.py"]);
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Unrelated Local Script",
            desktop_entry: None,
        },
        &sender_with_arguments("/usr/bin/python3", runtime_identity, &["/tmp/local.py"]),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Unresolved);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn dynamic_only_contract_is_unverified_instead_of_suspicious() {
    let (runtime_path, runtime_identity) = installed_system_executable();
    let mut first = system_record(
        "org.example.Dynamic",
        "Dynamic App",
        &runtime_path,
        runtime_identity,
    );
    first
        .launch_spec
        .as_mut()
        .expect("dynamic launch spec")
        .arguments = vec![LaunchArgument::FieldCode(FieldCode::Files)];
    let second = system_record(
        "org.example.Other",
        "Other App",
        &runtime_path,
        runtime_identity,
    );
    let index = DesktopIdentityIndex::from_records(vec![first, second], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Dynamic App",
            desktop_entry: Some("org.example.Dynamic"),
        },
        &sender_with_arguments(&runtime_path, runtime_identity, &["/tmp/payload"]),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_eq!(
        resolution.diagnostics.verification,
        LaunchVerificationView::InsufficientEvidence
    );
    assert_eq!(
        resolution.diagnostics.launch_authority,
        LaunchAuthorityView::DynamicOnly
    );
    assert_eq!(
        resolution.diagnostics.matched_desktop_id,
        "org.example.Dynamic"
    );
}
