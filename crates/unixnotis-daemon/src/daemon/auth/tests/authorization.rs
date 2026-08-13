use std::collections::HashMap;
use std::fs::File;
use std::os::fd::OwnedFd;
use zbus::Message;

#[cfg(target_os = "linux")]
use super::authorization::required_linux_process_fd;
use super::authorization::{
    authorize_control_call, authorize_interaction_call, authorize_panel_readiness_call,
    authorize_popup_readiness_call, control_executable_error, control_owner_uid_error,
};
#[cfg(target_os = "linux")]
use super::credentials::CallerCredentials;
use super::executable_trust::paths::canonicalize_best_effort;
use super::policy::TRUSTED_INTERACTION_EXECUTABLES;
use super::support::write_executable;
use crate::test_support::{daemon_state_for_test, TempRoot};

fn open_test_executable(path: &std::path::Path) -> OwnedFd {
    File::open(path).expect("open test executable").into()
}

fn message_without_bus_sender() -> Message {
    // Locally built messages have no unique bus sender, which must fail auth early
    Message::method("/", "Ping")
        .expect("method call builder")
        .build(&())
        .expect("method call message")
}

#[tokio::test]
async fn control_authorization_rejects_header_without_bus_sender() {
    let state = daemon_state_for_test(false).await;
    let message = message_without_bus_sender();
    let header = message.header();

    let err = authorize_control_call(&state, &header, "TestControl")
        .await
        .expect_err("missing sender must be rejected");

    assert!(err.to_string().contains("missing sender"));
}

#[tokio::test]
async fn panel_readiness_authorization_rejects_header_without_bus_sender() {
    let state = daemon_state_for_test(false).await;
    let message = message_without_bus_sender();
    let header = message.header();

    let err = authorize_panel_readiness_call(&state, &header, "PanelReady")
        .await
        .expect_err("missing sender must be rejected");

    assert!(err.to_string().contains("missing sender"));
}

#[tokio::test]
async fn interaction_authorization_rejects_header_without_bus_sender() {
    let state = daemon_state_for_test(false).await;
    let message = message_without_bus_sender();
    let header = message.header();

    let err = authorize_interaction_call(&state, &header, "InvokeAction")
        .await
        .expect_err("missing interaction sender must be rejected");

    assert!(err.to_string().contains("missing sender"));
}

#[tokio::test]
async fn popup_readiness_authorization_rejects_header_without_bus_sender() {
    let state = daemon_state_for_test(false).await;
    let message = message_without_bus_sender();
    let header = message.header();

    let err = authorize_popup_readiness_call(&state, &header, "PopupsReady")
        .await
        .expect_err("missing sender must be rejected");

    assert!(err.to_string().contains("missing sender"));
}

#[test]
fn control_uid_error_is_none_only_for_matching_uid() {
    assert!(control_owner_uid_error(1000, 1000).is_none());

    let err = control_owner_uid_error(1001, 1000).expect("mismatched uid should fail");

    assert!(err.to_string().contains("caller uid"));
}

#[test]
fn control_executable_error_rejects_missing_or_untrusted_binary() {
    let root = TempRoot::new("auth-executable-error");
    let untrusted_name = root.join(".local/bin/noticenterctl");
    write_executable(&untrusted_name);
    let untrusted_name = canonicalize_best_effort(&untrusted_name);
    let untrusted_fd = open_test_executable(&untrusted_name);

    // An allowed executable name still fails when its file object is outside
    // the trusted build or install tree
    assert!(control_executable_error(
        Some(&untrusted_name),
        Some(&untrusted_fd),
        &["noticenterctl"],
        true,
        &HashMap::new(),
    )
    .is_some());
    assert!(control_executable_error::<OwnedFd>(
        None,
        None::<&OwnedFd>,
        &["noticenterctl"],
        true,
        &HashMap::new(),
    )
    .is_some());
    assert!(control_executable_error(
        Some(&untrusted_name),
        Some(&untrusted_fd),
        &["unknown"],
        true,
        &HashMap::new(),
    )
    .is_some());
}

#[test]
fn interaction_executable_policy_rejects_untrusted_components() {
    let root = TempRoot::new("auth-interaction-executable");
    for executable in ["unixnotis-center", "unixnotis-popups"] {
        let path = root.join(".local/bin").join(executable);
        write_executable(&path);
        let path = canonicalize_best_effort(&path);
        let fd = open_test_executable(&path);

        // Renderer names do not create trust for arbitrary local-bin files
        assert!(control_executable_error::<OwnedFd>(
            Some(&path),
            Some(&fd),
            &TRUSTED_INTERACTION_EXECUTABLES,
            true,
            &HashMap::new(),
        )
        .is_some());
    }

    let cli = root.join(".local/bin/noticenterctl");
    write_executable(&cli);
    let cli = canonicalize_best_effort(&cli);
    let cli_fd = open_test_executable(&cli);

    // The CLI is not an interactive renderer, even when its name is allowed
    // by another control policy
    assert!(control_executable_error::<OwnedFd>(
        Some(&cli),
        Some(&cli_fd),
        &TRUSTED_INTERACTION_EXECUTABLES,
        true,
        &HashMap::new(),
    )
    .is_some());
}

#[test]
fn trial_control_authorization_rejects_all_arbitrary_local_bin_components() {
    // Every privileged component name still requires a trusted-tree executable
    let root = TempRoot::new("auth-local-bin-components");

    for executable in [
        "noticenterctl",
        "unixnotis-center",
        "unixnotis-popups",
        "unixnotis-daemon",
    ] {
        let forged = root.join(".local/bin").join(executable);
        write_executable(&forged);
        let forged_path = canonicalize_best_effort(&forged);
        let forged_fd = open_test_executable(&forged_path);

        assert!(control_executable_error(
            Some(&forged_path),
            Some(&forged_fd),
            &[executable],
            true,
            &HashMap::new(),
        )
        .is_some());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_authorization_rejects_credentials_without_a_stable_process_handle() {
    let credentials = CallerCredentials::default();

    let error = required_linux_process_fd(&credentials)
        .expect_err("Linux authorization must fail without ProcessFD");

    assert!(error.to_string().contains("stable process handle"));
}
