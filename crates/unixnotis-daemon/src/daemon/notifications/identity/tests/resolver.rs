use std::collections::HashSet;
use std::path::PathBuf;

use unixnotis_core::{AttributionClass, InlineReplyPolicy};

use super::*;
use crate::daemon::notifications::identity::desktop_index::{DesktopIdentityIndex, DesktopRecord};
use crate::daemon::notifications::identity::FileIdentity;

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
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Allow);
    assert!(!resolution.attribution.source_label.contains("unverified"));
}

#[test]
fn user_desktop_identity_requires_confirmation_instead_of_immediate_reply() {
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
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Confirm);
    assert!(!resolution.attribution.has_warning());
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
