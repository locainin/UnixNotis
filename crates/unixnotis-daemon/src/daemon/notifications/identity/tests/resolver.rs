use std::collections::HashSet;
use std::path::PathBuf;

use unixnotis_core::{AttributionClass, InlineReplyPolicy};

use super::*;
use crate::daemon::notifications::identity::desktop_index::model::{
    ExecutableIdentity, LaunchArgument, LaunchSpec, LiteralArgument,
};
use crate::daemon::notifications::identity::desktop_index::{DesktopIdentityIndex, DesktopRecord};
use crate::daemon::notifications::identity::FileIdentity;

trait DesktopRecordFixture {
    fn fixture(
        id: &str,
        display_name: &str,
        executable_path: &str,
        identity: FileIdentity,
        system_entry: bool,
        dbus_activatable: bool,
    ) -> Self;

    fn with_launch_literals(self, arguments: &[&str]) -> Self;
}

impl DesktopRecordFixture for DesktopRecord {
    fn fixture(
        id: &str,
        display_name: &str,
        executable_path: &str,
        identity: FileIdentity,
        system_entry: bool,
        dbus_activatable: bool,
    ) -> Self {
        Self {
            id: id.to_string(),
            display_name: display_name.to_string(),
            badge_icon: id.to_string(),
            executable_path: Some(PathBuf::from(executable_path)),
            executable_identity: Some(identity),
            desktop_identity: Some(identity),
            system_origin: system_entry,
            system_association: system_entry,
            association_eligible: true,
            dbus_activatable,
            launch_spec: None,
            names: HashSet::from([normalize_name(display_name)]),
        }
    }

    fn with_launch_literals(mut self, arguments: &[&str]) -> Self {
        let executable = self
            .executable_identity
            .expect("launch fixture needs executable identity");
        self.launch_spec = Some(LaunchSpec {
            executable,
            arguments: arguments
                .iter()
                .map(|value| {
                    LaunchArgument::Literal(LiteralArgument {
                        value: value.as_bytes().to_vec(),
                        file: None,
                    })
                })
                .collect(),
            protected_literal_files: 1,
            literal_files_are_system_managed: true,
        });
        self
    }
}

trait DesktopIdentityIndexFixture {
    fn from_records(
        records: Vec<DesktopRecord>,
        trusted_relays: Vec<(PathBuf, FileIdentity)>,
    ) -> Self;

    fn with_trusted_portal(self, path: PathBuf, identity: FileIdentity) -> Self;
}

impl DesktopIdentityIndexFixture for DesktopIdentityIndex {
    fn from_records(
        records: Vec<DesktopRecord>,
        trusted_relays: Vec<(PathBuf, FileIdentity)>,
    ) -> Self {
        let mut index = Self::default();
        for record in records {
            index.index_record(record);
        }
        index.trusted_relays = trusted_relays
            .into_iter()
            .map(|(path, identity)| ExecutableIdentity { path, identity })
            .collect();
        index
    }

    fn with_trusted_portal(mut self, path: PathBuf, identity: FileIdentity) -> Self {
        index_trusted_portal(&mut self, path, identity);
        self
    }
}

fn index_trusted_portal(index: &mut DesktopIdentityIndex, path: PathBuf, identity: FileIdentity) {
    index
        .trusted_portals
        .push(ExecutableIdentity { path, identity });
}

fn identity(device: u64, inode: u64, uid: u32) -> FileIdentity {
    FileIdentity {
        device,
        inode,
        uid,
        mode: 0o100_755,
    }
}

fn sender(path: &str, identity: FileIdentity) -> SenderMetadata {
    SenderMetadata {
        sender_name: Some(":1.42".to_string()),
        sender_executable: Some(path.to_string()),
        sender_executable_identity: Some(identity),
        ..SenderMetadata::default()
    }
}

fn sender_with_arguments(path: &str, identity: FileIdentity, arguments: &[&str]) -> SenderMetadata {
    let mut metadata = sender(path, identity);
    metadata.sender_cmdline = Some(
        std::iter::once(path)
            .chain(arguments.iter().copied())
            .map(|argument| argument.as_bytes().to_vec())
            .collect(),
    );
    metadata
}

fn system_record(id: &str, name: &str, path: &str, identity: FileIdentity) -> DesktopRecord {
    DesktopRecord::fixture(id, name, path, identity, true, false)
}

#[test]
fn system_desktop_identity_allows_legitimate_signal_reply() {
    let signal_identity = identity(1, 10, 0);
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.signal.Signal",
            "Signal",
            "/usr/bin/signal-desktop",
            signal_identity,
        )],
        Vec::new(),
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Signal",
            desktop_entry: Some("org.signal.Signal.desktop"),
        },
        &sender("/usr/bin/signal-desktop", signal_identity),
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

    assert_ne!(
        resolution.attribution.class,
        AttributionClass::SystemAssociated
    );
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
            resolution.attribution.class,
            AttributionClass::SystemAssociated,
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

    assert_ne!(
        resolution.attribution.class,
        AttributionClass::SystemAssociated
    );
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn matching_fixed_system_application_argument_allows_association() {
    let runtime_identity = identity(52, 520, 0);
    let record = system_record(
        "org.example.ScriptApp",
        "Script App",
        "/usr/bin/pypy3",
        runtime_identity,
    )
    .with_launch_literals(&["/usr/share/script-app/main.py"]);
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Script App",
            desktop_entry: Some("org.example.ScriptApp"),
        },
        &sender_with_arguments(
            "/usr/bin/pypy3",
            runtime_identity,
            &["/usr/share/script-app/main.py"],
        ),
        &index,
        &HashSet::new(),
    );

    assert_eq!(
        resolution.attribution.class,
        AttributionClass::SystemAssociated
    );
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
            resolution.attribution.class,
            AttributionClass::SystemAssociated,
            "{executable} accepted a different no-hint application payload"
        );
        assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    }
}

#[test]
fn no_hint_shared_runtime_with_matching_protected_payload_is_allowed() {
    let runtime_identity = identity(61, 610, 0);
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
            reported_name: "Example Password Manager",
            desktop_entry: None,
        },
        &sender_with_arguments(
            "/usr/bin/python3",
            runtime_identity,
            &["/usr/share/password-manager/main.py"],
        ),
        &index,
        &HashSet::new(),
    );

    assert_eq!(
        resolution.attribution.class,
        AttributionClass::SystemAssociated
    );
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

    assert_eq!(resolution.attribution.class, AttributionClass::Unknown);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn unmediated_flatpak_process_cannot_become_portal_associated() {
    let flatpak_identity = identity(21, 210, 0);
    let mut record = system_record(
        "org.example.FlatpakApp",
        "Flatpak App",
        "/usr/bin/flatpak",
        flatpak_identity,
    );
    record.association_eligible = false;
    record.system_association = false;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Flatpak App",
            desktop_entry: Some("org.example.FlatpakApp"),
        },
        &sender("/usr/bin/flatpak", flatpak_identity),
        &index,
        &HashSet::new(),
    );

    assert_ne!(
        resolution.attribution.class,
        AttributionClass::PortalAssociated
    );
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn an_empty_app_name_does_not_turn_an_untrusted_relay_into_a_portal() {
    let flatpak_identity = identity(24, 240, 0);
    let relay_identity = identity(25, 250, 0);
    let mut record = system_record(
        "org.example.FlatpakApp",
        "Flatpak App",
        "/usr/bin/flatpak",
        flatpak_identity,
    );
    record.association_eligible = false;
    record.system_association = false;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "",
            desktop_entry: Some("org.example.FlatpakApp"),
        },
        &sender("/usr/lib/untrusted-relay", relay_identity),
        &index,
        &HashSet::new(),
    );

    assert_ne!(
        resolution.attribution.class,
        AttributionClass::PortalAssociated
    );
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn portal_mediated_flatpak_uses_broker_verified_desktop_identity() {
    let flatpak_identity = identity(22, 220, 0);
    let portal_identity = identity(23, 230, 0);
    let mut record = system_record(
        "org.example.FlatpakApp",
        "Flatpak App",
        "/usr/bin/flatpak",
        flatpak_identity,
    );
    record.association_eligible = false;
    record.system_association = false;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new()).with_trusted_portal(
        PathBuf::from("/usr/lib/xdg-desktop-portal-gtk"),
        portal_identity,
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            // The GTK portal backend forwards an empty app name and verified desktop-entry hint
            reported_name: "",
            desktop_entry: Some("org.example.FlatpakApp"),
        },
        &sender("/usr/lib/xdg-desktop-portal-gtk", portal_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(
        resolution.attribution.class,
        AttributionClass::PortalAssociated
    );
    assert_eq!(resolution.attribution.display_name, "Flatpak App");
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Allow);
}

#[test]
fn user_shadow_cannot_join_the_system_desktop_group() {
    let system_identity = identity(30, 300, 0);
    let user_identity = identity(31, 310, 1000);
    let system = system_record(
        "org.signal.Signal",
        "Signal",
        "/usr/bin/signal-desktop",
        system_identity,
    );
    let mut user = DesktopRecord::fixture(
        "org.signal.Signal",
        "Signal",
        "/home/user/bin/signal",
        user_identity,
        false,
        false,
    );
    user.desktop_identity = Some(identity(32, 320, 1000));
    let index = DesktopIdentityIndex::from_records(vec![user, system], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Signal",
            desktop_entry: Some("org.signal.Signal"),
        },
        &sender("/home/user/bin/signal", user_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(
        resolution.attribution.class,
        AttributionClass::UserAssociated
    );
    assert!(resolution.attribution.has_warning());
    assert!(resolution
        .attribution
        .group_key
        .starts_with("user-desktop:"));
    assert_ne!(
        resolution.attribution.group_key,
        "system-desktop:org.signal.Signal"
    );
}

#[test]
fn visually_confusable_system_brand_is_a_conflict() {
    let signal_identity = identity(40, 400, 0);
    let hostile_identity = identity(41, 410, 1000);
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.signal.Signal",
            "Signal",
            "/usr/bin/signal-desktop",
            signal_identity,
        )],
        Vec::new(),
    );

    for claim in ["Sіgnal", "Signaⅼ"] {
        let resolution = resolve_with_evidence(
            AppClaim {
                reported_name: claim,
                desktop_entry: None,
            },
            &sender("/tmp/fake", hostile_identity),
            &index,
            &HashSet::new(),
        );

        assert_eq!(resolution.attribution.class, AttributionClass::Conflict);
        assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    }
}

#[test]
fn basename_spoof_is_conflicting_and_cannot_join_or_reply_as_signal() {
    let signal_identity = identity(1, 10, 0);
    let hostile_identity = identity(7, 70, 1000);
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.signal.Signal",
            "Signal",
            "/usr/bin/signal-desktop",
            signal_identity,
        )],
        Vec::new(),
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Signal",
            desktop_entry: None,
        },
        &sender("/tmp/signal-desktop", hostile_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::Conflict);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_ne!(
        resolution.attribution.group_key,
        "desktop:org.signal.Signal"
    );
}

#[test]
fn exact_keepassxc_name_spoof_never_becomes_system_associated() {
    let keepass_identity = identity(2, 20, 0);
    let hostile_identity = identity(8, 80, 1000);
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.keepassxc.KeePassXC",
            "KeePassXC",
            "/usr/bin/keepassxc",
            keepass_identity,
        )],
        Vec::new(),
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "KeePassXC",
            desktop_entry: None,
        },
        &sender("/tmp/keepassxc", hostile_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::Conflict);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn exact_system_notify_send_identity_is_a_non_replying_relay() {
    let relay_identity = identity(3, 30, 0);
    let index = DesktopIdentityIndex::from_records(
        Vec::new(),
        vec![(PathBuf::from("/usr/bin/notify-send"), relay_identity)],
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Screenshot",
            desktop_entry: None,
        },
        &sender("/usr/bin/notify-send", relay_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::TrustedRelay);
    assert_eq!(resolution.attribution.display_name, "Screenshot");
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert!(!resolution.attribution.has_warning());
    assert!(!resolution.attribution.source_label.contains("unverified"));
}

#[test]
fn trusted_relay_claiming_a_system_app_keeps_the_relay_class_and_adds_a_warning() {
    let signal_identity = identity(1, 10, 0);
    let relay_identity = identity(3, 30, 0);
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.signal.Signal",
            "Signal",
            "/usr/bin/signal-desktop",
            signal_identity,
        )],
        vec![(PathBuf::from("/usr/bin/notify-send"), relay_identity)],
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Signal",
            desktop_entry: None,
        },
        &sender("/usr/bin/notify-send", relay_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::TrustedRelay);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert!(resolution.attribution.has_warning());
    assert_ne!(
        resolution.attribution.group_key,
        "desktop:org.signal.Signal"
    );
}

#[test]
fn malicious_notify_send_basename_is_not_a_trusted_relay() {
    let real_relay = identity(3, 30, 0);
    let hostile_identity = identity(9, 90, 1000);
    let index = DesktopIdentityIndex::from_records(
        Vec::new(),
        vec![(PathBuf::from("/usr/bin/notify-send"), real_relay)],
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Screenshot",
            desktop_entry: None,
        },
        &sender("/tmp/notify-send", hostile_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::Unknown);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn owned_dbus_application_name_cannot_replace_executable_association() {
    let app_identity = identity(4, 40, 0);
    let mut record = DesktopRecord::fixture(
        "org.example.App",
        "Example App",
        "/usr/bin/example-app",
        app_identity,
        true,
        true,
    );
    record.executable_identity = None;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());
    let owned = HashSet::from(["org.example.app".to_string()]);

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.App"),
        },
        &sender("/usr/lib/example-launcher", identity(5, 50, 0)),
        &index,
        &owned,
    );

    assert_eq!(resolution.attribution.class, AttributionClass::Conflict);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert!(resolution
        .attribution
        .source_label
        .contains("bus name ownership lacks executable association"));
}

#[test]
fn desktop_id_validation_never_accepts_a_path_or_control_character() {
    assert_eq!(
        validate_desktop_id("org.signal.Signal.desktop").as_deref(),
        Some("org.signal.Signal")
    );
    assert_eq!(validate_desktop_id("../signal"), None);
    assert_eq!(validate_desktop_id("org.example.\nApp"), None);
    assert_eq!(validate_desktop_id("."), None);
    assert_eq!(validate_desktop_id(".desktop"), None);
    assert_eq!(
        validate_desktop_id(&"a".repeat(256)).map(|id| id.len()),
        Some(256)
    );
    assert_eq!(validate_desktop_id(&"a".repeat(257)), None);
}
